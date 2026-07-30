//! Installing components from a registry.
//!
//! `copit add @my-kit/auth` resolves the component's dependencies, shows the plan, then
//! copies each component with the ordinary fetch machinery before handing any package
//! dependencies to the project's package manager. Components are copied
//! dependency-first, so a component can import one that is already on disk.

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::cli::AddCommand;
use crate::config::{self, CopitConfig, ResolvedSettings};
use crate::installers;
use crate::registry::{Deps, InstallPlan, PlannedComponent, RegistryIndex};
use crate::sources::{self, Source};

use super::common::{self, portable_display, should_write_existing};

/// Install every requested registry component, sharing one plan and confirmation.
pub async fn run(
    registry_sources: &[Source],
    cmd: &AddCommand,
    settings: &ResolvedSettings,
    cfg: &CopitConfig,
) -> Result<()> {
    // Group by registry: each has its own index, target and variants.
    let mut by_registry: BTreeMap<&String, Vec<String>> = BTreeMap::new();
    for source in registry_sources {
        let Source::Registry {
            registry,
            component,
        } = source
        else {
            bail!("Not a registry source: {}", source.to_source_string());
        };
        by_registry
            .entry(registry)
            .or_default()
            .push(component.clone());
    }

    for (name, components) in by_registry {
        install_from(name, &components, cmd, settings, cfg).await?;
    }

    Ok(())
}

async fn install_from(
    registry_name: &str,
    requested: &[String],
    cmd: &AddCommand,
    settings: &ResolvedSettings,
    cfg: &CopitConfig,
) -> Result<()> {
    let (registry, index) = super::registry::open(registry_name).await?;

    let variants = if cmd.variants.is_empty() {
        registry.variants.clone()
    } else {
        cmd.variants.clone()
    };

    let installed = config::installed_components(cfg, registry_name);

    let deps = if cmd.no_deps {
        Deps::Skip
    } else {
        Deps::Resolve
    };
    let plan = index.plan(requested, &variants, &cmd.with, &installed, deps)?;

    if plan.components.is_empty() {
        println!("Nothing to do: already installed.");
        return Ok(());
    }

    // The registry may suggest a directory, but only the user picks one: `registry add`
    // records the suggestion into `registries.<name>.target`, so by here every candidate
    // is local configuration.
    let target = cmd
        .to
        .clone()
        .or_else(|| registry.target.clone())
        .unwrap_or_else(|| cfg.target.clone());
    common::validate_install_target(&target)?;

    let root = std::env::current_dir().context("Failed to read the current directory")?;
    let manager = resolve_manager(&registry, &index, &root);

    // Show what will actually happen: --no-packages means nothing is installed, so
    // naming a manager here would be a lie.
    let display_manager = if cmd.no_packages {
        None
    } else {
        manager.as_deref()
    };
    print_plan(
        &plan,
        &target,
        &variants,
        &cmd.with,
        display_manager,
        cmd.no_packages,
    );

    if cmd.dry_run {
        println!("\nDry run: nothing was written.");
        return Ok(());
    }

    if !cmd.yes && !confirm(&plan)? {
        println!("Cancelled.");
        return Ok(());
    }

    let install = Install {
        registry_name,
        registry_source: &registry.source,
        index: &index,
        target: &target,
        variants: &variants,
        licenses_dir: cfg.licenses_dir.as_deref(),
        cmd,
        settings,
    };

    for planned in &plan.components {
        copy_component(&install, planned).await?;
    }

    if !cmd.no_packages && !plan.packages.is_empty() {
        install_packages(&plan.packages, manager.as_deref(), &root)?;
    }

    Ok(())
}

/// Which package manager to use, honouring explicit configuration over detection.
fn resolve_manager(
    registry: &config::RegistryConfig,
    index: &RegistryIndex,
    root: &Path,
) -> Option<String> {
    match registry.package_manager.as_deref() {
        Some("none") => None,
        Some(name) => Some(name.to_string()),
        None => {
            let ecosystem = (!index.ecosystem.is_empty()).then_some(index.ecosystem.as_str());
            installers::detect(root, ecosystem).map(|installer| installer.name.to_string())
        }
    }
}

fn print_plan(
    plan: &InstallPlan,
    target: &str,
    variants: &[String],
    optional: &[String],
    manager: Option<&str>,
    skipping_packages: bool,
) {
    println!();
    for planned in &plan.components {
        let component = &planned.component;
        let marker = if planned.requested {
            ""
        } else {
            "  (required)"
        };
        println!(
            "  {}  {}  ({}){}",
            component.name, component.version, component.tier, marker
        );
        if !component.description.is_empty() {
            println!("    {}", component.description);
        }
    }

    println!("\n  Files -> {target}/");
    for planned in &plan.components {
        println!(
            "    {}/  ({} file{})",
            component_dir(&planned.component),
            planned.files.len(),
            if planned.files.len() == 1 { "" } else { "s" }
        );
    }

    if !variants.is_empty() {
        println!("\n  Variants: {}", variants.join(", "));
    }
    if !optional.is_empty() {
        println!("  Including: {}", optional.join(", "));
    }

    if !plan.packages.is_empty() {
        let note = match (skipping_packages, manager) {
            (true, _) => "not installed, --no-packages",
            (false, Some(name)) => name,
            (false, None) => "install these yourself",
        };
        let label = if skipping_packages || manager.is_none() {
            format!("Packages ({note})")
        } else {
            format!("Packages (via {note})")
        };
        println!("\n  {label}: {}", plan.packages.join(", "));
    }
}

fn confirm(plan: &InstallPlan) -> Result<bool> {
    let components = plan.components.len();
    let packages = plan.packages.len();

    let prompt = if packages == 0 {
        format!(
            "\nInstall {components} component{}?",
            if components == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "\nInstall {components} component{} and {packages} package{}?",
            if components == 1 { "" } else { "s" },
            if packages == 1 { "" } else { "s" }
        )
    };

    // Treating "no terminal" as "no" made `copit add` in CI print Cancelled and exit 0,
    // so a pipeline that meant to install something silently installed nothing.
    dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(true)
        .interact()
        .map_err(|_| {
            anyhow::anyhow!("Cannot prompt for confirmation here. Re-run with --yes, or --dry-run.")
        })
}

/// Everything one install shares across its components.
struct Install<'a> {
    registry_name: &'a str,
    registry_source: &'a str,
    index: &'a RegistryIndex,
    target: &'a str,
    variants: &'a [String],
    licenses_dir: Option<&'a str>,
    cmd: &'a AddCommand,
    settings: &'a ResolvedSettings,
}

/// Copy one component, reusing the normal GitHub/HTTP fetch path.
async fn copy_component(install: &Install<'_>, planned: &PlannedComponent) -> Result<()> {
    let Install {
        registry_name,
        registry_source,
        index,
        target,
        variants,
        licenses_dir,
        cmd,
        settings,
    } = install;
    let component = &planned.component;

    let track_path = PathBuf::from(target).join(component_dir(component));
    let track_key = portable_display(&track_path);
    let backlink = format!("{registry_name}:{}", component.name);

    // Refuse to adopt a path someone else already owns: silently stamping the backlink
    // on would redirect that entry's future updates to this registry.
    if let Some(existing) = config::get_source_entry(&track_key) {
        match existing.component.as_deref() {
            Some(owner) if owner == backlink => {}
            Some(owner) => bail!(
                "{track_key} is already tracked as component '{owner}'. \
                 Remove it first, or install to a different target."
            ),
            None => bail!(
                "{track_key} is already tracked from {}. \
                 Remove it first, or install to a different target.",
                existing.source
            ),
        }
    }

    let fetched = crate::registry::fetch_component(registry_source, &component.path)
        .await
        .with_context(|| format!("Failed to fetch component '{}'", component.name))?;

    let publish: HashSet<&String> = planned.files.iter().collect();

    let mut written = 0;
    for (within, contents) in &fetched.files {
        // Only publish what the index lists: excludes and unselected variants are
        // already applied there, so tests and metadata never reach the user.
        if !publish.contains(within) {
            continue;
        }

        let dest = track_path.join(within);
        common::validate_no_path_traversal(&dest, target)?;

        if !should_write_existing(&dest, settings.overwrite, settings.skip)? {
            continue;
        }

        common::write_file(&dest, contents)?;
        written += 1;
    }

    // Some languages need every directory to be a package; the registry says which
    // file, if any, makes that true.
    if let Some(marker) = &index.install.package_marker {
        let marker_path = track_path.join(marker);
        if !marker_path.exists() {
            common::write_file(&marker_path, b"")?;
        }
        let target_marker = PathBuf::from(target).join(marker);
        if !target_marker.exists() {
            common::write_file(&target_marker, b"")?;
        }
    }

    if written == 0 {
        println!("  {}: already up to date", component.name);
    } else {
        println!("  {}: {} file(s) -> {}", component.name, written, track_key);
    }

    if !cmd.no_license {
        common::write_license_files(&fetched.license_files, &track_path, target, *licenses_dir)?;
    }

    // Track even when nothing was written: the files are on disk either way, and an
    // untracked component can never be updated and is re-planned on every install.
    let source_string = crate::registry::component_source(registry_source, &component.path);

    let (version_ref, commit) = match sources::parse_source(&source_string) {
        Ok(Source::GitHub {
            owner,
            repo,
            version,
            ..
        }) => (
            Some(version.clone()),
            sources::github::resolve_commit_sha(&owner, &repo, &version).await,
        ),
        _ => (None, None),
    };

    config::add_source_entry(
        &track_key,
        &source_string,
        version_ref.as_deref(),
        commit.as_deref(),
        cmd.freeze.then_some(true),
        cmd.no_license.then_some(true),
    )?;
    config::set_source_component(&track_key, &backlink, variants)?;

    Ok(())
}

/// Directory a component is copied into.
///
/// Taken from the registry path, so it is the importable name the component's own
/// relative imports expect. Both the plan and the copy must agree on this.
pub fn component_dir(component: &crate::registry::Component) -> String {
    Path::new(&component.path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| component.name.clone())
}

fn install_packages(packages: &[String], manager: Option<&str>, root: &Path) -> Result<()> {
    let Some(name) = manager else {
        println!(
            "\nNo package manager detected. Install these yourself:\n  {}",
            packages.join(" ")
        );
        return Ok(());
    };

    let Some(installer) = installers::by_name(name) else {
        bail!("Unknown package manager '{name}' in copit.toml");
    };

    println!();
    installers::install(installer, packages, root)
}
