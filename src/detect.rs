//! Selecting a registry's variants from the project's `package.json` / `pyproject.toml`.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::registry::RegistryIndex;

/// Variants detected for a project, and the manifests that decided it.
#[derive(Debug, Default, PartialEq)]
pub struct Detected {
    pub variants: Vec<String>,
    pub manifests: Vec<PathBuf>,
}

/// Detect `index`'s variants from manifests between `root.join(target)` and `root`.
pub fn detect_variants(index: &RegistryIndex, root: &Path, target: Option<&str>) -> Detected {
    if index.detect.is_empty() {
        return Detected::default();
    }

    let start = target.map_or_else(|| root.to_path_buf(), |target| root.join(target));
    let mut packages = HashSet::new();
    let mut manifests = Vec::new();

    for dir in start.ancestors().filter(|dir| dir.starts_with(root)) {
        for (file, read) in manifest_readers(&index.ecosystem) {
            let path = dir.join(file);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let found = read(&text);
            if !found.is_empty() {
                packages.extend(found);
                manifests.push(path);
            }
        }
    }

    let variants = index
        .variants
        .iter()
        .filter(|variant| {
            index.detect.get(*variant).is_some_and(|rule| {
                rule.packages
                    .iter()
                    .any(|package| packages.contains(&normalise(package)))
            })
        })
        .cloned()
        .collect();

    Detected {
        variants,
        manifests,
    }
}

type Reader = fn(&str) -> BTreeSet<String>;

fn manifest_readers(ecosystem: &str) -> Vec<(&'static str, Reader)> {
    let node: (&str, Reader) = ("package.json", package_json_packages);
    let python: (&str, Reader) = ("pyproject.toml", pyproject_packages);
    match ecosystem {
        "node" => vec![node],
        "python" => vec![python],
        _ => vec![node, python],
    }
}

/// Every dependency name in a `package.json`, dev and peer included.
pub fn package_json_packages(text: &str) -> BTreeSet<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return BTreeSet::new();
    };
    [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ]
    .iter()
    .filter_map(|table| json.get(table)?.as_object())
    .flat_map(|table| table.keys().map(|name| normalise(name)))
    .collect()
}

/// Every dependency name in a `pyproject.toml`: project, optional, groups and Poetry.
pub fn pyproject_packages(text: &str) -> BTreeSet<String> {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return BTreeSet::new();
    };
    let mut specs: Vec<String> = Vec::new();

    let strings = |value: &toml::Value| -> Vec<String> {
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    if let Some(project) = doc.get("project") {
        if let Some(deps) = project.get("dependencies") {
            specs.extend(strings(deps));
        }
        if let Some(extras) = project
            .get("optional-dependencies")
            .and_then(|v| v.as_table())
        {
            extras.values().for_each(|deps| specs.extend(strings(deps)));
        }
    }
    if let Some(groups) = doc.get("dependency-groups").and_then(|v| v.as_table()) {
        groups.values().for_each(|deps| specs.extend(strings(deps)));
    }

    let mut names: BTreeSet<String> = specs.iter().map(|spec| requirement_name(spec)).collect();

    let poetry = doc
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.as_table());
    if let Some(poetry) = poetry {
        let tables = ["dependencies", "dev-dependencies"]
            .iter()
            .filter_map(|key| poetry.get(*key)?.as_table().cloned())
            .chain(
                poetry
                    .get("group")
                    .and_then(|groups| groups.as_table())
                    .into_iter()
                    .flat_map(|groups| groups.values())
                    .filter_map(|group| group.get("dependencies")?.as_table().cloned()),
            );
        for table in tables {
            names.extend(table.keys().map(|name| normalise(name)));
        }
    }

    names.retain(|name| !name.is_empty() && name != "python");
    names
}

/// The distribution name of a PEP 508 requirement, e.g. `Django[argon2]>=5` -> `django`.
fn requirement_name(spec: &str) -> String {
    let end = spec
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(spec.len());
    normalise(&spec[..end])
}

/// Case-insensitive, with `-`, `_` and `.` equal for Python names (PEP 503).
fn normalise(name: &str) -> String {
    let lowered = name.trim().to_ascii_lowercase();
    if lowered.starts_with('@') {
        return lowered;
    }
    lowered.replace(['_', '.'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn index(ecosystem: &str) -> RegistryIndex {
        serde_json::from_value(serde_json::json!({
            "version": 1,
            "name": "ui",
            "ecosystem": ecosystem,
            "variants": ["react", "vue", "django", "fastapi"],
            "detect": {
                "react": { "packages": ["react"] },
                "vue": { "packages": ["vue"] },
                "django": { "packages": ["Django", "channels"] },
                "fastapi": { "packages": ["fastapi"] }
            },
            "components": {}
        }))
        .unwrap()
    }

    #[test]
    fn package_json_lists_every_dependency_table() {
        let found = package_json_packages(
            r#"{ "dependencies": { "react": "^19" }, "devDependencies": { "@types/react": "^19" },
                 "peerDependencies": { "vue": ">=3" } }"#,
        );
        assert_eq!(
            found,
            BTreeSet::from(["react".into(), "@types/react".into(), "vue".into()])
        );
    }

    #[test]
    fn pyproject_reads_requirements_extras_groups_and_poetry() {
        let found = pyproject_packages(
            r#"
[project]
dependencies = ["Django[argon2]>=5.0", "chanx>=2"]
[project.optional-dependencies]
api = ["fastapi ; python_version >= '3.11'"]
[dependency-groups]
dev = ["pytest"]
[tool.poetry.dependencies]
python = "^3.11"
Channels_Redis = "*"
"#,
        );
        assert_eq!(
            found,
            BTreeSet::from([
                "django".into(),
                "chanx".into(),
                "fastapi".into(),
                "pytest".into(),
                "channels-redis".into()
            ])
        );
    }

    #[test]
    fn detects_from_the_manifest_nearest_the_target() {
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("web")).unwrap();
        std::fs::write(
            root.path().join("web/package.json"),
            r#"{ "dependencies": { "react": "^19" } }"#,
        )
        .unwrap();

        let found = detect_variants(&index("node"), root.path(), Some("web/src/kits"));

        assert_eq!(found.variants, vec!["react".to_string()]);
        assert_eq!(found.manifests, vec![root.path().join("web/package.json")]);
    }

    #[test]
    fn a_node_registry_ignores_pyproject() {
        let root = tempfile::TempDir::new().unwrap();
        std::fs::write(
            root.path().join("pyproject.toml"),
            "[project]\ndependencies = [\"fastapi\"]\n",
        )
        .unwrap();

        assert_eq!(
            detect_variants(&index("node"), root.path(), None),
            Detected::default()
        );
        assert_eq!(
            detect_variants(&index("python"), root.path(), None).variants,
            vec!["fastapi".to_string()]
        );
    }

    #[test]
    fn nothing_is_detected_without_rules() {
        let mut plain = index("node");
        plain.detect = BTreeMap::new();
        let root = tempfile::TempDir::new().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{ "dependencies": { "react": "^19" } }"#,
        )
        .unwrap();

        assert!(detect_variants(&plain, root.path(), None)
            .variants
            .is_empty());
    }
}
