//! The `copit registry`, `copit search` and `copit info` commands.
//!
//! Configuring a registry records where its index lives, so components can then be
//! installed by id (`@name/component`) instead of by path.

use anyhow::{bail, Context, Result};

use crate::cli::{InfoCommand, RegistryAddCommand, SearchCommand};
use crate::config::{self, RegistryConfig};
use crate::installers;
use crate::registry::{load_index, Component, RegistryIndex};
use crate::sources::Source;

use super::common;

/// Strip a leading `@` from a registry reference.
fn registry_name(reference: &str) -> &str {
    reference.strip_prefix('@').unwrap_or(reference)
}

/// Reject names that cannot survive a round trip through `@name/component` and the
/// `registry:component` backlink.
///
/// A name containing `:` used to install fine and then fail every later update with
/// "registry 'gh' is no longer configured", naming a registry the user never added.
fn validate_registry_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Registry name cannot be empty");
    }
    if let Some(bad) = name
        .chars()
        .find(|c| matches!(c, ':' | '/' | '@') || c.is_whitespace())
    {
        bail!("Registry name '{name}' cannot contain '{bad}'");
    }
    Ok(())
}

/// Look up a configured registry and load its index.
pub async fn open(name: &str) -> Result<(RegistryConfig, RegistryIndex)> {
    let config =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    let registry = config.registries.get(name).cloned().ok_or_else(|| {
        let configured: Vec<&str> = config.registries.keys().map(String::as_str).collect();
        if configured.is_empty() {
            anyhow::anyhow!(
                "No registry named '{name}'. Configure one first:\n  \
                 copit registry add {name} github:owner/repo@ref"
            )
        } else {
            anyhow::anyhow!(
                "No registry named '{name}'. Configured: {}",
                configured.join(", ")
            )
        }
    })?;

    let index = load_index(&registry.source).await?;
    Ok((registry, index))
}

/// Resolve a `Source::Registry` to its configured registry and index.
pub async fn open_for(source: &Source) -> Result<(String, RegistryConfig, RegistryIndex)> {
    match source {
        Source::Registry { registry, .. } => {
            let (config, index) = open(registry).await?;
            Ok((registry.clone(), config, index))
        }
        other => bail!("Not a registry source: {}", other.to_source_string()),
    }
}

/// Run `copit registry add`.
pub async fn add(cmd: &RegistryAddCommand) -> Result<()> {
    if !config::config_exists() {
        bail!("No copit.toml in the current directory. Run `copit init` first.");
    }

    validate_registry_name(&cmd.name)?;

    // `@name/component` names a component *inside* a registry, so it can never be where
    // a registry lives. Rejecting it here keeps the internal "must be resolved before
    // fetching" error from reaching the user.
    if cmd.source.starts_with('@') {
        bail!(
            "'{}' is a component reference, not a registry location. \
             Use `github:owner/repo@ref` or a local directory.",
            cmd.source
        );
    }

    // Load the index now rather than at first use, so a typo in the source is caught
    // here instead of surfacing later as a failed install.
    let index = load_index(&cmd.source)
        .await
        .with_context(|| format!("Could not read a registry at '{}'", cmd.source))?;

    index.check_variants(&cmd.variants)?;

    if let Some(manager) = &cmd.package_manager {
        if manager != "none" && installers::by_name(manager).is_none() {
            let known: Vec<&str> = installers::INSTALLERS.iter().map(|i| i.name).collect();
            bail!(
                "Unknown package manager '{manager}'. Known: {}, or 'none'",
                known.join(", ")
            );
        }
    }

    // The index may suggest a directory, but a downloaded file must not silently steer
    // where later installs write. Record the suggestion now so it is visible in
    // copit.toml and the user can edit it.
    let target = match (&cmd.to, &index.install.target) {
        (Some(to), _) => Some(to.clone()),
        (None, Some(suggested)) => {
            println!("Registry '{}' suggests target '{suggested}'", cmd.name);
            Some(suggested.clone())
        }
        (None, None) => None,
    };
    if let Some(target) = &target {
        common::validate_install_target(target)?;
    }

    let entry = RegistryConfig {
        source: cmd.source.clone(),
        target,
        variants: cmd.variants.clone(),
        package_manager: cmd.package_manager.clone(),
    };
    config::upsert_registry(&cmd.name, &entry)?;

    println!(
        "Configured registry '{}' ({} component{}) from {}",
        cmd.name,
        index.components.len(),
        if index.components.len() == 1 { "" } else { "s" },
        cmd.source
    );
    if !cmd.variants.is_empty() {
        println!("  variants: {}", cmd.variants.join(", "));
    }
    println!("\nInstall a component with:");
    if let Some(first) = index.components.keys().next() {
        println!("  copit add @{}/{}", cmd.name, first);
    }

    Ok(())
}

/// Run `copit registry list`.
pub fn list() -> Result<()> {
    let config =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    if config.registries.is_empty() {
        println!("No registries configured. Add one with:");
        println!("  copit registry add <name> github:owner/repo@ref");
        return Ok(());
    }

    for (name, registry) in &config.registries {
        println!("@{name}");
        println!("  source:  {}", registry.source);
        if let Some(target) = &registry.target {
            println!("  target:  {target}");
        }
        if !registry.variants.is_empty() {
            println!("  variants: {}", registry.variants.join(", "));
        }
        if let Some(manager) = &registry.package_manager {
            println!("  packages: {manager}");
        }
    }

    Ok(())
}

/// Run `copit search`.
pub async fn search(cmd: &SearchCommand) -> Result<()> {
    let name = registry_name(&cmd.registry);
    let (_, index) = open(name).await?;

    let matches: Vec<&Component> = match &cmd.query {
        Some(query) => index.search(query),
        None => index.components.values().collect(),
    };

    if matches.is_empty() {
        println!(
            "No components in @{name} match '{}'",
            cmd.query.as_deref().unwrap_or("")
        );
        return Ok(());
    }

    let width = matches
        .iter()
        .map(|component| component.name.len())
        .max()
        .unwrap_or(0);

    for component in matches {
        println!(
            "  {:<width$}  {:<8}  {}",
            component.name,
            component.tier,
            component.description,
            width = width
        );
    }

    Ok(())
}

/// Run `copit info`.
pub async fn info(cmd: &InfoCommand) -> Result<()> {
    let source = crate::sources::parse_source(&cmd.component)?;
    let (registry_name, registry, index) = open_for(&source).await?;

    let Source::Registry { component: id, .. } = &source else {
        unreachable!("open_for rejects non-registry sources");
    };

    let component = index.component(id)?;
    let variants = &registry.variants;

    println!("@{registry_name}/{}", component.name);
    if !component.title.is_empty() {
        println!("  {}", component.title);
    }
    if !component.description.is_empty() {
        println!("  {}", component.description);
    }
    println!();
    println!("  tier:      {}", component.tier);
    println!("  version:   {}", component.version);
    if !component.tags.is_empty() {
        println!("  tags:      {}", component.tags.join(", "));
    }
    if !component.authors.is_empty() {
        println!("  authors:   {}", component.authors.join(", "));
    }
    if let Some(homepage) = &index.homepage {
        println!("  homepage:  {homepage}");
    }
    if !component.requires.is_empty() {
        println!("  requires:  {}", component.requires.join(", "));
    }

    let packages = component.packages_for(variants);
    if !packages.is_empty() {
        println!("  packages:  {}", packages.join(", "));
    }

    let files = component.files_for(variants);
    println!("\n  files ({}):", files.len());
    for file in &files {
        println!("    {file}");
    }

    Ok(())
}
