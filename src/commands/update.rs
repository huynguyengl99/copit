//! The `copit update` command.
//!
//! Re-fetches specific tracked sources by path, optionally changing the
//! version ref.

use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::cli::UpdateCommand;
use crate::commands::common::should_write_existing;
use crate::config::{self, ResolvedSettings, SourceEntry};
use crate::sources::{self, Source};

use super::common;

/// Run the `update` command for the given paths.
pub async fn run(cmd: &UpdateCommand) -> Result<()> {
    if cmd.paths.is_empty() {
        bail!("No paths specified. Usage: copit update <path>...");
    }

    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    for path in &cmd.paths {
        let entry = cfg
            .sources
            .iter()
            .find(|e| e.path == *path)
            .ok_or_else(|| anyhow::anyhow!("Source not found in copit.toml: {path}"))?;

        let frozen = if cmd.freeze {
            Some(true)
        } else if cmd.unfreeze {
            Some(false)
        } else {
            None
        };

        if entry.frozen == Some(true) && !cmd.unfreeze {
            println!("Skipping frozen: {}", entry.path);
            continue;
        }

        let settings =
            ResolvedSettings::resolve(cmd.overwrite, cmd.skip, cmd.backup, Some(entry), &cfg);

        update_entry(entry, cmd.version_ref.as_deref(), settings, frozen, &cfg).await?;
    }

    Ok(())
}

/// Re-fetch one tracked entry, through its registry when it came from one.
///
/// Every caller must go through here: a component re-fetched as a plain path drags in
/// the tests and metadata the index withheld, and for a local registry its recorded
/// source does not even parse.
pub async fn update_entry(
    entry: &SourceEntry,
    ref_override: Option<&str>,
    settings: ResolvedSettings,
    frozen: Option<bool>,
    cfg: &config::CopitConfig,
) -> Result<()> {
    if entry.component.is_some() {
        return update_component(entry, ref_override, settings, frozen, cfg).await;
    }

    update_source(
        entry,
        ref_override,
        settings,
        frozen,
        &cfg.target,
        cfg.licenses_dir.as_deref(),
    )
    .await
}

/// Re-fetch a component through its registry rather than as a plain path: the index
/// decides which files are published, so a plain re-fetch would drag in the tests and
/// metadata that installing excluded.
async fn update_component(
    entry: &SourceEntry,
    ref_override: Option<&str>,
    settings: ResolvedSettings,
    frozen: Option<bool>,
    cfg: &config::CopitConfig,
) -> Result<()> {
    let backlink = entry
        .component
        .as_deref()
        .expect("caller checked the backlink is present");
    let (registry_name, component_id) = backlink
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("Malformed component backlink '{backlink}'"))?;

    let registry = cfg.registries.get(registry_name).ok_or_else(|| {
        anyhow::anyhow!(
            "'{}' came from registry '{registry_name}', which is no longer configured. \
             Re-add it with `copit registry add {registry_name} <source>`.",
            entry.path
        )
    })?;

    // A new ref applies to the registry as a whole: the index and the component must
    // come from the same commit, or the published file list may not match the files.
    let registry_source = match ref_override {
        Some(new_ref) => crate::registry::with_ref(&registry.source, new_ref)?,
        None => registry.source.clone(),
    };

    let index = crate::registry::load_index(&registry_source, registry.index.as_deref()).await?;
    let component = index.component(component_id)?;

    let fetched = crate::registry::fetch_component(&registry_source, &component.path).await?;
    if fetched.files.is_empty() {
        println!("No files found for {}", entry.path);
        return Ok(());
    }

    // Variants recorded at install time win: `--variant` can override the registry's
    // list, and reproducing that selection is the only way the same adapter stays
    // current instead of going stale beside a freshly written one.
    let variants = if entry.variants.is_empty() {
        &registry.variants
    } else {
        &entry.variants
    };

    // Only an entry that recorded nothing at all picks up the registry's groups, which
    // is how turning `optional` on reaches installs that predate it.
    let recorded: Option<Vec<String>> = entry
        .optional
        .clone()
        .or_else(|| (!registry.optional.is_empty()).then(|| registry.optional.clone()));
    let optional: &[String] = recorded.as_deref().unwrap_or_default();

    let published: HashSet<String> = component.files_for(variants).into_iter().collect();

    let selected: HashSet<String> = component.optional_files(optional).into_iter().collect();

    // Entries predating `optional` recorded no selection, so fall back to refreshing
    // whichever optional files are already on disk. Anything in neither set is never
    // written: a file that merely exists locally must not be overwritten.
    let unrecorded: HashSet<String> = if recorded.is_none() {
        component
            .optional
            .values()
            .flat_map(crate::registry::OptionalGroup::include)
            .cloned()
            .collect()
    } else {
        HashSet::new()
    };

    let track_path = PathBuf::from(&entry.path);
    let mut updated = 0;

    for (within, contents) in &fetched.files {
        let dest = track_path.join(within);
        common::validate_no_path_traversal(&dest, &entry.path)?;

        let is_opted_in =
            selected.contains(within) || (unrecorded.contains(within) && dest.exists());
        if !published.contains(within) && !is_opted_in {
            continue;
        }

        if common::handle_excludes(
            &dest,
            &track_path,
            &entry.excludes,
            contents,
            settings.backup,
        )? {
            continue;
        }

        if !should_write_existing(&dest, settings.overwrite, settings.skip)? {
            continue;
        }

        common::write_file(&dest, contents)?;
        updated += 1;
    }

    let version = match entry.component_version.as_deref() {
        Some(before) if before != component.version => {
            format!("{before} -> {}", component.version)
        }
        _ => component.version.clone(),
    };

    println!(
        "Updated {} ({} file{}) from @{registry_name}/{component_id} {version}",
        entry.path,
        updated,
        if updated == 1 { "" } else { "s" },
    );

    if entry.no_license != Some(true) {
        common::write_license_files(
            &fetched.license_files,
            &track_path,
            &entry.path,
            cfg.licenses_dir.as_deref(),
        )?;
    }

    let new_source = crate::registry::component_source(&registry_source, &component.path);
    let (version_ref, commit) = match crate::sources::parse_source(&new_source) {
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
        &entry.path,
        &new_source,
        version_ref.as_deref(),
        commit.as_deref(),
        frozen,
        entry.no_license,
    )?;
    config::set_source_component(
        &entry.path,
        backlink,
        super::registry_add::component_version(component),
        variants,
        recorded.as_deref(),
    )?;

    install_group_requires(
        registry_name,
        &registry_source,
        &index,
        component,
        entry,
        variants,
        optional,
        settings,
        cfg,
    )
    .await
}

/// Install components a selected group requires but the project does not have yet.
///
/// A group can declare `requires` in a later index version, so an install that was
/// complete when it ran no longer is. Packages are only reported, since `update` has
/// never run a package manager and has no flag to opt out of one.
#[allow(clippy::too_many_arguments)]
async fn install_group_requires(
    registry_name: &str,
    registry_source: &str,
    index: &crate::registry::RegistryIndex,
    component: &crate::registry::Component,
    entry: &SourceEntry,
    variants: &[String],
    optional: &[String],
    settings: ResolvedSettings,
    cfg: &config::CopitConfig,
) -> Result<()> {
    let required = component.optional_requires(optional);
    if required.is_empty() {
        return Ok(());
    }

    // Not `cfg`: an earlier entry in the same run may have installed it already.
    let installed = config::installed_components(&config::load_config()?, registry_name);
    let missing: Vec<String> = required
        .into_iter()
        .filter(|id| !installed.contains(id))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    // Beside the requiring component, which is where the original install put them
    // even when `--to` overrode the configured target.
    let target = match PathBuf::from(&entry.path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => common::portable_display(parent),
        _ => cfg.target.clone(),
    };
    common::validate_install_target(&target)?;

    let plan = index.plan(
        &missing,
        variants,
        optional,
        &installed,
        crate::registry::Deps::Resolve,
    )?;

    println!(
        "  --with {} now requires: {}",
        optional.join(", "),
        missing.join(", ")
    );

    let install = super::registry_add::Install {
        registry_name,
        registry_source,
        index,
        target: &target,
        variants,
        optional: Some(optional),
        licenses_dir: cfg.licenses_dir.as_deref(),
        no_license: entry.no_license == Some(true),
        freeze: false,
        settings: &settings,
    };

    for planned in &plan.components {
        super::registry_add::copy_component(&install, planned).await?;
    }

    if !plan.packages.is_empty() {
        println!("  Packages needed: {}", plan.packages.join(", "));
    }

    Ok(())
}

/// Re-fetch a single tracked source, optionally overriding the version ref.
///
/// `settings` controls file-handling behavior (overwrite, skip, backup),
/// already resolved from CLI flags, per-source config, and config defaults.
///
/// `frozen` controls the frozen flag in `copit.toml`: `Some(true)` sets it,
/// `Some(false)` removes it, and `None` leaves it unchanged.
///
/// Also used by [`super::update_all`] to update each source during a full all updates.
pub async fn update_source(
    entry: &SourceEntry,
    ref_override: Option<&str>,
    settings: ResolvedSettings,
    frozen: Option<bool>,
    target: &str,
    licenses_dir: Option<&str>,
) -> Result<()> {
    let source = sources::parse_source(&entry.source)?;

    let source = match ref_override {
        Some(new_ref) => source.with_version(new_ref),
        None => source,
    };

    let fetch_result = super::add::fetch_source(&source).await?;
    let files = fetch_result.files;

    if files.is_empty() {
        println!("No files found for {}", source.to_source_string());
        return Ok(());
    }

    let track_path = PathBuf::from(&entry.path);
    let suggested = source.suggested_name();
    let is_single = files.len() == 1;
    let strip_prefix = common::compute_strip_prefix(&suggested, !is_single);

    // Determine the base target directory
    let base_target = track_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    for (relative_path, contents) in &files {
        let dest = if is_single {
            track_path.clone()
        } else {
            common::compute_dest(
                relative_path,
                &base_target,
                &suggested,
                &strip_prefix,
                false,
            )
        };

        // Check if this file is in excludes
        if common::handle_excludes(
            &dest,
            &track_path,
            &entry.excludes,
            contents,
            settings.backup,
        )? {
            continue;
        }

        if !should_write_existing(&dest, settings.overwrite, settings.skip)? {
            continue;
        }

        common::write_file(&dest, contents)?;
        println!("Updated: {}", common::portable_display(&dest));
    }

    // Resolve version ref and commit for GitHub sources
    let (version_ref, commit) = match &source {
        Source::GitHub {
            owner,
            repo,
            version,
            ..
        } => {
            let sha = sources::github::resolve_commit_sha(owner, repo, version).await;
            (Some(version.clone()), sha)
        }
        _ => (None, None),
    };

    config::add_source_entry(
        &entry.path,
        &source.to_source_string(),
        version_ref.as_deref(),
        commit.as_deref(),
        frozen,
        None,
    )?;

    let license_files = fetch_result.license_files;
    if entry.no_license != Some(true) && !license_files.is_empty() {
        common::write_license_files(&license_files, &track_path, target, licenses_dir)?;
    }

    Ok(())
}
