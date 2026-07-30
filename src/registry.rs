//! Registry index model and dependency resolution.
//!
//! A *registry* is a repository that publishes named **components**: folders of source
//! code a user installs by id rather than by path.
//!
//! ```bash
//! copit add @my-kit/auth
//! ```
//!
//! The registry publishes a generated `registry.json` describing every component: where
//! it lives, which other components it requires, and which packages it needs. copit
//! fetches that one file, resolves the graph, and then reuses the ordinary fetch/copy
//! machinery for each component. The registry's declared `ecosystem` picks the package
//! installer; see [`crate::installers`].
//!
//! An index is downloaded, so it is data, never code: components declare files,
//! dependencies and text, never commands for copit to run.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// The index format version this build understands.
pub const SUPPORTED_VERSION: u32 = 1;

/// Per-variant additions to a component.
///
/// A variant is a registry-defined second axis, typically a framework, where the same
/// component ships a different adapter per target.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Variant {
    /// Extra packages required when this variant is selected.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Files copied *only* when this variant is selected.
    #[serde(default)]
    pub include: Vec<String>,
}

/// One installable component.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Component {
    /// Registry-unique id, e.g. `auth`.
    pub name: String,
    /// Human-readable name.
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Free-form label such as `core` or `contrib`. copit treats it as opaque.
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub version: String,
    /// Path within the registry repository.
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    /// Other components in this registry, resolved transitively.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Ecosystem packages this component needs.
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub variants: BTreeMap<String, Variant>,
    /// Files the registry expects to be copied, excludes already applied.
    #[serde(default)]
    pub files: Vec<String>,
    /// Named groups of files excluded from a normal install, e.g. `tests`, copied on
    /// `--with`.
    #[serde(default)]
    pub optional: BTreeMap<String, Vec<String>>,
}

impl Component {
    /// Packages needed for this component under the selected variants.
    pub fn packages_for(&self, variants: &[String]) -> Vec<String> {
        let mut packages = self.dependencies.clone();
        for variant in variants {
            if let Some(config) = self.variants.get(variant) {
                packages.extend(config.dependencies.iter().cloned());
            }
        }
        packages
    }

    /// Files to copy under the selected variants. Variant files are additive: one
    /// listed under an unselected variant is left behind, unless a selected variant
    /// includes it too.
    pub fn files_for(&self, variants: &[String]) -> Vec<String> {
        let selected: HashSet<&String> = variants.iter().collect();

        let wanted: HashSet<&String> = self
            .variants
            .iter()
            .filter(|(name, _)| selected.contains(name))
            .flat_map(|(_, config)| config.include.iter())
            .collect();

        let variant_only: HashSet<&String> = self
            .variants
            .iter()
            .filter(|(name, _)| !selected.contains(name))
            .flat_map(|(_, config)| config.include.iter())
            .filter(|file| !wanted.contains(*file))
            .collect();

        self.files
            .iter()
            .filter(|file| !variant_only.contains(file))
            .cloned()
            .collect()
    }

    /// Files from the named optional groups, listed outside [`Component::files`]
    /// because they are opt-in.
    pub fn optional_files(&self, groups: &[String]) -> Vec<String> {
        let mut files: Vec<String> = groups
            .iter()
            .filter_map(|group| self.optional.get(group))
            .flatten()
            .cloned()
            .collect();
        files.sort();
        files.dedup();
        files
    }
}

/// Installation defaults declared by the registry.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InstallConfig {
    /// Suggested target directory.
    #[serde(default)]
    pub target: Option<String>,
    /// File to create in every directory copit creates, so the target is importable
    /// in languages that need it (Python's `__init__.py`).
    #[serde(default)]
    pub package_marker: Option<String>,
    /// Generator-side default: glob patterns the registry never publishes. copit copies
    /// exactly what each component lists in `files`, so this is not applied at install.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Named groups excluded by default, installable on request (e.g. `tests`).
    #[serde(default)]
    pub optional: BTreeMap<String, Vec<String>>,
}

/// A fetched registry index.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryIndex {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Where the registry says it lives, e.g. `github:owner/repo`. Informational only:
    /// components are fetched from the source the user configured, at the ref they
    /// pinned. See [`fetch_component`].
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Package ecosystem, mapped to an installer by [`crate::installers`].
    #[serde(default)]
    pub ecosystem: String,
    #[serde(default)]
    pub variants: Vec<String>,
    #[serde(default)]
    pub install: InstallConfig,
    pub components: BTreeMap<String, Component>,
}

/// One component in an install plan, with variants already applied.
#[derive(Debug, Clone)]
pub struct PlannedComponent {
    pub component: Component,
    /// Files to copy, variant filtering and requested optional groups applied.
    pub files: Vec<String>,
    /// True when the user asked for this component rather than it being pulled in.
    pub requested: bool,
}

/// Whether an install plan follows each component's `requires`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deps {
    Resolve,
    /// `--no-deps`: install only what was named.
    Skip,
}

/// What an `add` will do, in dependency order.
#[derive(Debug, Clone, Default)]
pub struct InstallPlan {
    /// Components to install, dependencies before dependents.
    pub components: Vec<PlannedComponent>,
    /// Merged, de-duplicated packages across every component.
    pub packages: Vec<String>,
}

impl RegistryIndex {
    /// Parse an index, rejecting formats this build does not understand.
    pub fn from_json(raw: &str) -> Result<Self> {
        let index: RegistryIndex = serde_json::from_str(raw)
            .map_err(|error| anyhow::anyhow!("Invalid registry index: {error}"))?;

        if index.version != SUPPORTED_VERSION {
            bail!(
                "Registry '{}' uses index version {}, but this copit supports version {}. \
                 Upgrade copit, or pin an older registry ref.",
                index.name,
                index.version,
                SUPPORTED_VERSION
            );
        }

        // Lookups go through the map key but the backlink is written from `name`, so a
        // generator that lets them drift produces components that install and can then
        // never be found again by `update`.
        for (id, component) in &index.components {
            if &component.name != id {
                bail!(
                    "Registry '{}' lists component '{id}' with name '{}'. \
                     The key and the name must match.",
                    index.name,
                    component.name
                );
            }
        }

        Ok(index)
    }

    /// Look up a component, suggesting alternatives when the id is unknown.
    pub fn component(&self, id: &str) -> Result<&Component> {
        if let Some(component) = self.components.get(id) {
            return Ok(component);
        }

        let suggestions: Vec<&str> = self
            .components
            .keys()
            .filter(|name| is_near(name, id))
            .map(String::as_str)
            .take(3)
            .collect();

        if suggestions.is_empty() {
            bail!(
                "Registry '{}' has no component '{}'. Run `copit search @{}` to list them.",
                self.name,
                id,
                self.name
            );
        }

        bail!(
            "Registry '{}' has no component '{}'. Did you mean: {}?",
            self.name,
            id,
            suggestions.join(", ")
        )
    }

    /// Validate the selected variants against what the registry declares.
    pub fn check_variants(&self, variants: &[String]) -> Result<()> {
        for variant in variants {
            if !self.variants.contains(&variant.to_string()) {
                bail!(
                    "Registry '{}' has no variant '{}'. Available: {}",
                    self.name,
                    variant,
                    if self.variants.is_empty() {
                        "none".to_string()
                    } else {
                        self.variants.join(", ")
                    }
                );
            }
        }
        Ok(())
    }

    /// Build an install plan for `requested`.
    ///
    /// Dependencies come before the components that need them, so a component can
    /// import one that is already on disk. Components in `installed` are skipped
    /// unless requested directly. `deps` decides whether `requires` is followed;
    /// validation and de-duplication apply either way.
    pub fn plan(
        &self,
        requested: &[String],
        variants: &[String],
        optional: &[String],
        installed: &HashSet<String>,
        deps: Deps,
    ) -> Result<InstallPlan> {
        self.check_variants(variants)?;
        self.check_optional(optional)?;

        for id in requested {
            self.component(id)?;
        }

        let mut resolver = Resolver {
            index: self,
            variants,
            optional,
            installed,
            wanted: requested.iter().map(String::as_str).collect(),
            deps,
            done: HashSet::new(),
            visiting: Vec::new(),
            plan: InstallPlan::default(),
        };

        for id in requested {
            resolver.visit(id)?;
        }

        let mut plan = resolver.plan;
        let mut seen: HashSet<String> = HashSet::new();
        plan.packages = plan
            .components
            .iter()
            .flat_map(|planned| planned.component.packages_for(variants))
            .filter(|package| seen.insert(package.clone()))
            .collect();

        Ok(plan)
    }

    /// Validate requested optional groups against what the registry declares.
    ///
    /// Index-level `install.optional` is a generator-side default: only groups a
    /// component actually materialised into its own `optional` map can be copied, so
    /// only those are accepted here.
    pub fn check_optional(&self, groups: &[String]) -> Result<()> {
        for group in groups {
            let known = self
                .components
                .values()
                .any(|component| component.optional.contains_key(group));
            if !known {
                let mut available: Vec<&str> = self
                    .components
                    .values()
                    .flat_map(|component| component.optional.keys())
                    .map(String::as_str)
                    .collect();
                available.sort_unstable();
                available.dedup();
                bail!(
                    "Registry '{}' has no optional group '{}'. Available: {}",
                    self.name,
                    group,
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                );
            }
        }
        Ok(())
    }

    /// Components whose id, title, description or tags match `query`.
    pub fn search(&self, query: &str) -> Vec<&Component> {
        let needle = query.to_lowercase();
        self.components
            .values()
            .filter(|component| {
                component.name.to_lowercase().contains(&needle)
                    || component.title.to_lowercase().contains(&needle)
                    || component.description.to_lowercase().contains(&needle)
                    || component
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&needle))
            })
            .collect()
    }
}

/// Traversal state for one [`RegistryIndex::plan`] call.
struct Resolver<'a> {
    index: &'a RegistryIndex,
    variants: &'a [String],
    optional: &'a [String],
    installed: &'a HashSet<String>,
    wanted: HashSet<&'a str>,
    deps: Deps,
    done: HashSet<String>,
    visiting: Vec<String>,
    plan: InstallPlan,
}

impl Resolver<'_> {
    fn visit(&mut self, id: &str) -> Result<()> {
        if self.done.contains(id) {
            return Ok(());
        }

        if let Some(start) = self.visiting.iter().position(|seen| seen == id) {
            let mut cycle: Vec<&str> = self.visiting[start..].iter().map(String::as_str).collect();
            cycle.push(id);
            bail!(
                "Dependency cycle in registry '{}': {}",
                self.index.name,
                cycle.join(" -> ")
            );
        }

        let component = self.index.component(id)?;

        if self.deps == Deps::Resolve {
            self.visiting.push(id.to_string());
            // Post-order: dependencies land in the plan before whatever needs them.
            for dependency in component.requires.clone() {
                self.visit(&dependency)?;
            }
            self.visiting.pop();
        }

        self.done.insert(id.to_string());

        let requested = self.wanted.contains(id);
        if self.installed.contains(id) && !requested {
            return Ok(());
        }

        let mut files = component.files_for(self.variants);
        files.extend(component.optional_files(self.optional));

        self.plan.components.push(PlannedComponent {
            component: component.clone(),
            files,
            requested,
        });

        Ok(())
    }
}

/// Whether `candidate` is close enough to `typed` to suggest it.
///
/// Substring matching alone misses a typo anywhere in the word, so an edit distance of
/// one (a wrong, missing, extra or transposed letter) counts too. Transpositions matter
/// most: `cache` for `cache` shares only two leading characters, so a prefix rule would
/// never have suggested it.
fn is_near(candidate: &str, typed: &str) -> bool {
    if candidate.contains(typed) || typed.contains(candidate) {
        return true;
    }

    within_one_edit(&candidate.to_lowercase(), &typed.to_lowercase())
}

/// Whether one string becomes the other with a single insert, delete, substitution or
/// transposition of adjacent characters (Damerau-Levenshtein distance of one).
fn within_one_edit(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }

    let shared = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[shared..], &b[shared..]);

    if a.is_empty() || b.is_empty() {
        // One ran out first, so the remainder is a single insert or delete.
        return a.len() + b.len() <= 1;
    }

    // Substitution, or a transposition of the two characters that now differ.
    if a[1..] == b[1..] {
        return true;
    }
    if a.len() == b.len() && a.len() >= 2 && a[0] == b[1] && a[1] == b[0] && a[2..] == b[2..] {
        return true;
    }

    a[1..] == *b || *a == b[1..]
}

/// Filename of a registry's generated index, at the root of its source.
pub const INDEX_FILE: &str = "registry.json";

/// Load a registry index from a configured source.
///
/// A local filesystem path is accepted so a registry author can test against their
/// working tree before publishing.
pub async fn load_index(source: &str) -> Result<RegistryIndex> {
    let local = std::path::Path::new(source);
    if local.is_dir() {
        let path = local.join(INDEX_FILE);
        let raw = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "No {} in {}. Generate the index before installing from a local registry.",
                INDEX_FILE,
                local.display()
            )
        })?;
        return RegistryIndex::from_json(&raw);
    }

    let index_source = format!("{}/{}", source.trim_end_matches('/'), INDEX_FILE);
    let parsed = crate::sources::parse_source(&index_source)
        .with_context(|| format!("Invalid registry source '{source}'"))?;

    let fetched = crate::commands::add::fetch_source(&parsed)
        .await
        .with_context(|| format!("Failed to fetch {index_source}"))?;

    let (_, contents) = fetched
        .files
        .first()
        .ok_or_else(|| anyhow::anyhow!("Registry index {index_source} is empty"))?;

    let raw = String::from_utf8(contents.clone()).context("Registry index is not valid UTF-8")?;

    RegistryIndex::from_json(&raw)
}

/// Fetch one component's files, keyed by path relative to the component directory.
///
/// Uses the *configured* registry source, not the `source` the index declares about
/// itself: the user pinned a ref and components must come from that same ref, whereas
/// the index's own `source` often has no ref at all.
pub async fn fetch_component(
    registry_source: &str,
    component_path: &str,
) -> Result<crate::commands::add::FetchResult> {
    let local = std::path::Path::new(registry_source);
    if local.is_dir() {
        return Ok(crate::commands::add::FetchResult {
            files: read_local_component(&local.join(component_path))?,
            license_files: read_local_licenses(local),
        });
    }

    let source_string = component_source(registry_source, component_path);
    let source = crate::sources::parse_source(&source_string)
        .with_context(|| format!("Invalid component source '{source_string}'"))?;

    let mut fetched = crate::commands::add::fetch_source(&source)
        .await
        .with_context(|| format!("Failed to fetch {source_string}"))?;

    // Fetched paths are repo-relative; make them component-relative so the copy lands
    // at <target>/<component>/... regardless of how deep the registry nests things.
    let prefix = format!("{}/", component_path.trim_matches('/'));
    fetched.files = fetched
        .files
        .into_iter()
        .map(|(path, contents)| match path.strip_prefix(&prefix) {
            Some(relative) => (relative.to_string(), contents),
            None => (path, contents),
        })
        .collect();

    Ok(fetched)
}

/// Point a registry source at a different ref.
///
/// A registry source has no path (`github:owner/repo@v1.0.0`), so it does not parse as
/// a [`crate::sources::Source`]. Rewriting the ref here is what makes `update --ref`
/// work; erroring is what stops it being silently ignored for sources that have no ref
/// to move, such as a local directory.
pub fn with_ref(registry_source: &str, new_ref: &str) -> Result<String> {
    if std::path::Path::new(registry_source).is_dir() {
        bail!(
            "--ref does not apply to the local registry at '{registry_source}'. \
             Point the registry at a tagged source first."
        );
    }

    for prefix in ["github:", "gh:"] {
        if let Some(rest) = registry_source.strip_prefix(prefix) {
            let (owner_repo, _) = rest.split_once('@').unwrap_or((rest, ""));
            return Ok(format!("{prefix}{owner_repo}@{new_ref}"));
        }
    }

    bail!("--ref is only supported for GitHub registries, not '{registry_source}'")
}

/// Where one component lives, as a source string built from the *configured* registry
/// source so the component comes from the ref the user pinned.
pub fn component_source(registry_source: &str, component_path: &str) -> String {
    format!(
        "{}/{}",
        registry_source.trim_end_matches('/'),
        component_path.trim_start_matches('/')
    )
}

/// License files at the root of a local registry, mirroring what the GitHub fetch
/// collects from a repository root.
fn read_local_licenses(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            let stem = name.split('.').next().unwrap_or(&name).to_uppercase();
            if path.is_file() && (stem == "LICENSE" || stem == "LICENCE" || stem == "COPYING") {
                Some((name, std::fs::read(&path).ok()?))
            } else {
                None
            }
        })
        .collect()
}

fn read_local_component(dir: &std::path::Path) -> Result<Vec<(String, Vec<u8>)>> {
    if !dir.is_dir() {
        bail!("Component directory not found: {}", dir.display());
    }

    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)
            .with_context(|| format!("Failed to read {}", current.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                // Build artefacts in an author's working tree are not part of the
                // component; a published archive would never contain them.
                let skip = path
                    .file_name()
                    .map(|name| name == "__pycache__" || name == ".git")
                    .unwrap_or(false);
                if !skip {
                    stack.push(path);
                }
                continue;
            }
            let relative = crate::commands::common::portable_display(
                path.strip_prefix(dir).expect("walked path is under dir"),
            );
            files.push((relative, std::fs::read(&path)?));
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(name: &str, requires: &[&str], deps: &[&str]) -> Component {
        Component {
            name: name.to_string(),
            title: name.to_string(),
            description: String::new(),
            tier: "core".to_string(),
            version: "0.1.0".to_string(),
            path: format!("kits/{name}"),
            tags: vec![],
            authors: vec![],
            requires: requires.iter().map(|r| r.to_string()).collect(),
            dependencies: deps.iter().map(|d| d.to_string()).collect(),
            variants: BTreeMap::new(),
            files: vec!["__init__.py".to_string()],
            optional: BTreeMap::new(),
        }
    }

    fn index(components: Vec<Component>) -> RegistryIndex {
        RegistryIndex {
            version: 1,
            name: "test".to_string(),
            title: "Test".to_string(),
            description: String::new(),
            source: "github:owner/repo".to_string(),
            homepage: None,
            ecosystem: "python".to_string(),
            variants: vec!["sqlite".to_string(), "postgres".to_string()],
            install: InstallConfig::default(),
            components: components
                .into_iter()
                .map(|component| (component.name.clone(), component))
                .collect(),
        }
    }

    fn plan_ids(plan: &InstallPlan) -> Vec<String> {
        plan.components
            .iter()
            .map(|planned| planned.component.name.clone())
            .collect()
    }

    #[test]
    fn dependencies_are_installed_before_dependents() {
        let registry = index(vec![
            component("store", &[], &[]),
            component("logger", &["store"], &[]),
        ]);

        let plan = registry
            .plan(
                &["logger".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(plan_ids(&plan), vec!["store", "logger"]);
    }

    #[test]
    fn transitive_dependencies_are_resolved() {
        let registry = index(vec![
            component("a", &["b"], &[]),
            component("b", &["c"], &[]),
            component("c", &[], &[]),
        ]);

        let plan = registry
            .plan(&["a".to_string()], &[], &[], &HashSet::new(), Deps::Resolve)
            .unwrap();

        assert_eq!(plan_ids(&plan), vec!["c", "b", "a"]);
    }

    #[test]
    fn a_shared_dependency_is_planned_once() {
        let registry = index(vec![
            component("shared", &[], &[]),
            component("one", &["shared"], &[]),
            component("two", &["shared"], &[]),
        ]);

        let plan = registry
            .plan(
                &["one".to_string(), "two".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(plan_ids(&plan), vec!["shared", "one", "two"]);
    }

    #[test]
    fn only_directly_requested_components_are_marked_requested() {
        let registry = index(vec![
            component("store", &[], &[]),
            component("logger", &["store"], &[]),
        ]);

        let plan = registry
            .plan(
                &["logger".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        let implied: Vec<&str> = plan
            .components
            .iter()
            .filter(|planned| !planned.requested)
            .map(|planned| planned.component.name.as_str())
            .collect();
        assert_eq!(implied, vec!["store"]);
    }

    #[test]
    fn already_installed_dependencies_are_skipped() {
        let registry = index(vec![
            component("store", &[], &[]),
            component("logger", &["store"], &[]),
        ]);
        let installed: HashSet<String> = ["store".to_string()].into_iter().collect();

        let plan = registry
            .plan(&["logger".to_string()], &[], &[], &installed, Deps::Resolve)
            .unwrap();

        assert_eq!(plan_ids(&plan), vec!["logger"]);
    }

    #[test]
    fn an_installed_component_is_replanned_when_asked_for_again() {
        let registry = index(vec![component("logger", &[], &[])]);
        let installed: HashSet<String> = ["logger".to_string()].into_iter().collect();

        let plan = registry
            .plan(&["logger".to_string()], &[], &[], &installed, Deps::Resolve)
            .unwrap();

        assert_eq!(plan_ids(&plan), vec!["logger"]);
    }

    #[test]
    fn a_cycle_is_reported_with_its_path() {
        let registry = index(vec![
            component("a", &["b"], &[]),
            component("b", &["a"], &[]),
        ]);

        let error = registry
            .plan(&["a".to_string()], &[], &[], &HashSet::new(), Deps::Resolve)
            .unwrap_err()
            .to_string();

        assert!(error.contains("Dependency cycle"), "{error}");
        assert!(error.contains("a -> b -> a"), "{error}");
    }

    #[test]
    fn a_self_dependency_is_a_cycle() {
        let registry = index(vec![component("a", &["a"], &[])]);

        assert!(registry
            .plan(&["a".to_string()], &[], &[], &HashSet::new(), Deps::Resolve)
            .is_err());
    }

    #[test]
    fn an_unknown_component_suggests_close_matches() {
        let registry = index(vec![component("analytics", &[], &[])]);

        let error = registry
            .plan(
                &["analy".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap_err()
            .to_string();

        assert!(error.contains("Did you mean"), "{error}");
        assert!(error.contains("analytics"), "{error}");
    }

    #[test]
    fn a_typo_suggests_the_component_it_resembles() {
        let registry = index(vec![
            component("analytics", &[], &[]),
            component("logger", &[], &[]),
        ]);

        let mut typos = one_edit_away("analytics");
        typos.push("analy".to_string()); // a bare prefix, matched as a substring

        for typo in typos {
            let error = registry
                .plan(
                    std::slice::from_ref(&typo),
                    &[],
                    &[],
                    &HashSet::new(),
                    Deps::Resolve,
                )
                .unwrap_err()
                .to_string();
            assert!(error.contains("analytics"), "{typo}: {error}");
        }
    }

    /// Single-edit variants of `word`: a transposition, a dropped letter and a doubled
    /// one. Built rather than spelled out because the repo lints for misspellings, and a
    /// linter that "fixes" a test fixture silently changes what the test asserts.
    fn one_edit_away(word: &str) -> Vec<String> {
        let mut transposed: Vec<char> = word.chars().collect();
        transposed.swap(2, 3);

        vec![
            transposed.into_iter().collect(),
            word[..word.len() - 1].to_string(),
            format!("{word}{}", word.chars().last().unwrap()),
        ]
    }

    #[test]
    fn a_typo_one_edit_away_finds_its_component() {
        // A transposition shares only two leading characters with the correct id, so the
        // old shared-prefix rule suggested nothing at all.
        let registry = index(vec![component("cache", &[], &[])]);

        for typo in one_edit_away("cache") {
            let error = registry
                .plan(
                    std::slice::from_ref(&typo),
                    &[],
                    &[],
                    &HashSet::new(),
                    Deps::Resolve,
                )
                .unwrap_err()
                .to_string();
            assert!(error.contains("Did you mean"), "{typo}: {error}");
            assert!(error.contains("cache"), "{typo}: {error}");
        }
    }

    #[test]
    fn a_component_name_must_match_its_key() {
        let raw = r#"{"version": 1, "name": "kit", "components": {
            "auth": {"name": "authentication", "path": "components/auth"}
        }}"#;

        let error = RegistryIndex::from_json(raw).unwrap_err().to_string();

        assert!(error.contains("key and the name must match"), "{error}");
    }

    #[test]
    fn an_unrelated_id_does_not_get_a_bogus_suggestion() {
        let registry = index(vec![component("analytics", &[], &[])]);

        let error = registry
            .plan(
                &["zzz".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap_err()
            .to_string();

        assert!(error.contains("copit search"), "{error}");
        assert!(!error.contains("Did you mean"), "{error}");
    }

    #[test]
    fn an_unknown_dependency_is_reported() {
        let registry = index(vec![component("a", &["ghost"], &[])]);

        let error = registry
            .plan(&["a".to_string()], &[], &[], &HashSet::new(), Deps::Resolve)
            .unwrap_err()
            .to_string();

        assert!(error.contains("ghost"), "{error}");
    }

    #[test]
    fn packages_merge_across_components_and_deduplicate() {
        let registry = index(vec![
            component("store", &[], &["redis>=5", "shared>=1"]),
            component("logger", &["store"], &["shared>=1"]),
        ]);

        let plan = registry
            .plan(
                &["logger".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(plan.packages, vec!["redis>=5", "shared>=1"]);
    }

    #[test]
    fn variant_packages_are_added_only_when_selected() {
        let mut widget = component("widget", &[], &["base>=1"]);
        widget.variants.insert(
            "sqlite".to_string(),
            Variant {
                dependencies: vec!["pg-driver>=5".to_string()],
                include: vec![],
            },
        );
        let registry = index(vec![widget]);

        let without = registry
            .plan(
                &["widget".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();
        let with = registry
            .plan(
                &["widget".to_string()],
                &["sqlite".to_string()],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(without.packages, vec!["base>=1"]);
        assert_eq!(with.packages, vec!["base>=1", "pg-driver>=5"]);
    }

    #[test]
    fn files_of_unselected_variants_are_not_copied() {
        let mut widget = component("widget", &[], &[]);
        widget.files = vec![
            "__init__.py".to_string(),
            "stores/sqlite.py".to_string(),
            "stores/postgres.py".to_string(),
        ];
        widget.variants.insert(
            "sqlite".to_string(),
            Variant {
                dependencies: vec![],
                include: vec!["stores/sqlite.py".to_string()],
            },
        );
        widget.variants.insert(
            "postgres".to_string(),
            Variant {
                dependencies: vec![],
                include: vec!["stores/postgres.py".to_string()],
            },
        );
        let registry = index(vec![widget]);

        let plan = registry
            .plan(
                &["widget".to_string()],
                &["sqlite".to_string()],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(
            plan.components[0].files,
            vec!["__init__.py", "stores/sqlite.py"]
        );
    }

    #[test]
    fn a_file_two_variants_share_is_kept_when_either_is_selected() {
        // A shared base module listed under both adapters must survive: excluding it
        // because the *other* variant also mentions it breaks the selected adapter.
        let mut widget = component("widget", &[], &[]);
        widget.files = vec![
            "__init__.py".to_string(),
            "stores/base.py".to_string(),
            "stores/sqlite.py".to_string(),
            "stores/postgres.py".to_string(),
        ];
        widget.variants.insert(
            "sqlite".to_string(),
            Variant {
                dependencies: vec![],
                include: vec!["stores/base.py".to_string(), "stores/sqlite.py".to_string()],
            },
        );
        widget.variants.insert(
            "postgres".to_string(),
            Variant {
                dependencies: vec![],
                include: vec![
                    "stores/base.py".to_string(),
                    "stores/postgres.py".to_string(),
                ],
            },
        );
        let registry = index(vec![widget]);

        let plan = registry
            .plan(
                &["widget".to_string()],
                &["sqlite".to_string()],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(
            plan.components[0].files,
            vec!["__init__.py", "stores/base.py", "stores/sqlite.py"]
        );
    }

    #[test]
    fn optional_groups_are_copied_only_when_requested() {
        let mut widget = component("widget", &[], &[]);
        widget.optional.insert(
            "tests".to_string(),
            vec!["tests/test_widget.py".to_string()],
        );
        let mut registry = index(vec![widget]);
        registry
            .install
            .optional
            .insert("tests".to_string(), vec!["tests/**".to_string()]);

        let without = registry
            .plan(
                &["widget".to_string()],
                &[],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();
        let with = registry
            .plan(
                &["widget".to_string()],
                &[],
                &["tests".to_string()],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap();

        assert_eq!(without.components[0].files, vec!["__init__.py"]);
        assert_eq!(
            with.components[0].files,
            vec!["__init__.py", "tests/test_widget.py"]
        );
    }

    #[test]
    fn an_unknown_optional_group_lists_what_is_available() {
        let mut widget = component("widget", &[], &[]);
        widget.optional.insert("tests".to_string(), vec![]);
        let registry = index(vec![widget]);

        let error = registry
            .plan(
                &["widget".to_string()],
                &[],
                &["docs".to_string()],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap_err()
            .to_string();

        assert!(error.contains("no optional group 'docs'"), "{error}");
        assert!(error.contains("tests"), "{error}");
    }

    #[test]
    fn an_unknown_variant_is_rejected() {
        let registry = index(vec![component("widget", &[], &[])]);

        let error = registry
            .plan(
                &["widget".to_string()],
                &["mysql".to_string()],
                &[],
                &HashSet::new(),
                Deps::Resolve,
            )
            .unwrap_err()
            .to_string();

        assert!(error.contains("no variant 'mysql'"), "{error}");
        assert!(error.contains("sqlite, postgres"), "{error}");
    }

    #[test]
    fn a_future_index_version_is_rejected_with_advice() {
        let raw = r#"{"version": 99, "name": "future", "source": "github:o/r", "components": {}}"#;

        let error = RegistryIndex::from_json(raw).unwrap_err().to_string();

        assert!(error.contains("version 99"), "{error}");
        assert!(error.contains("Upgrade copit"), "{error}");
    }

    #[test]
    fn a_minimal_index_parses() {
        let raw = r#"{
            "version": 1,
            "name": "mini",
            "source": "github:o/r",
            "components": {
                "one": {"name": "one", "path": "kits/one"}
            }
        }"#;

        let index = RegistryIndex::from_json(raw).unwrap();

        assert_eq!(index.components.len(), 1);
        assert_eq!(index.components["one"].path, "kits/one");
    }

    #[tokio::test]
    async fn a_local_registry_directory_is_loaded() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(INDEX_FILE),
            r#"{"version":1,"name":"local","source":"github:o/r","components":{}}"#,
        )
        .unwrap();

        let index = load_index(dir.path().to_str().unwrap()).await.unwrap();

        assert_eq!(index.name, "local");
    }

    #[test]
    fn a_ref_override_rewrites_a_bare_registry_source() {
        assert_eq!(
            with_ref("github:owner/repo@v1.0.0", "v2.0.0").unwrap(),
            "github:owner/repo@v2.0.0"
        );
        assert_eq!(
            with_ref("gh:owner/repo", "v2.0.0").unwrap(),
            "gh:owner/repo@v2.0.0"
        );
    }

    #[test]
    fn a_ref_override_is_rejected_rather_than_ignored() {
        // Silently dropping it is what made `update --ref` report success while
        // re-fetching the old ref.
        let dir = tempfile::TempDir::new().unwrap();

        let error = with_ref(dir.path().to_str().unwrap(), "v2.0.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("local registry"), "{error}");

        let error = with_ref("https://example.com/registry", "v2")
            .unwrap_err()
            .to_string();
        assert!(error.contains("only supported for GitHub"), "{error}");
    }

    #[test]
    fn an_optional_group_no_component_publishes_is_rejected() {
        // Index-level `install.optional` is a generator default; accepting it here
        // made `--with tests` print "Including: tests" and copy nothing.
        let mut registry = index(vec![component("widget", &[], &[])]);
        registry
            .install
            .optional
            .insert("tests".to_string(), vec!["tests/**".to_string()]);

        let error = registry
            .check_optional(&["tests".to_string()])
            .unwrap_err()
            .to_string();

        assert!(error.contains("no optional group 'tests'"), "{error}");
    }

    #[tokio::test]
    async fn a_local_directory_without_an_index_says_to_generate_it() {
        let dir = tempfile::TempDir::new().unwrap();

        let error = load_index(dir.path().to_str().unwrap())
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("registry.json"), "{error}");
    }

    #[test]
    fn search_matches_id_description_and_tags() {
        let mut cache = component("cache", &[], &[]);
        cache.description = "In-memory cache with a pluggable store".to_string();
        cache.tags = vec!["memory".to_string()];
        let registry = index(vec![cache, component("analytics", &[], &[])]);

        assert_eq!(registry.search("cache").len(), 1);
        assert_eq!(registry.search("pluggable").len(), 1);
        assert_eq!(registry.search("memory").len(), 1);
        assert_eq!(registry.search("nothing").len(), 0);
    }
}
