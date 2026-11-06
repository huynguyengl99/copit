//! Points a Go registry's import paths at the project module, since Go has no relative imports.

use std::borrow::Cow;
use std::path::Path;

/// The module path a `go.mod` declares.
pub fn module_of(go_mod: &str) -> Option<&str> {
    go_mod.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("module")?;
        let module = rest.trim().trim_matches('"');
        (!module.is_empty() && rest.starts_with(char::is_whitespace)).then_some(module)
    })
}

/// `contents` with `from/...` imports pointed at `target` inside the project in `root`.
///
/// Anything that is not a Go file, or a project without a `go.mod`, is left as is.
pub fn localize<'a>(
    contents: &'a [u8],
    file: &str,
    from: Option<&str>,
    target: &str,
    root: &Path,
) -> Cow<'a, [u8]> {
    let (Some(from), true) = (from, file.ends_with(".go")) else {
        return Cow::Borrowed(contents);
    };
    let Ok(go_mod) = std::fs::read_to_string(root.join("go.mod")) else {
        return Cow::Borrowed(contents);
    };
    let (Some(module), Ok(text)) = (module_of(&go_mod), std::str::from_utf8(contents)) else {
        return Cow::Borrowed(contents);
    };
    let target = target
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();
    let quoted = format!("\"{}/", from.trim_end_matches('/'));
    if !text.contains(&quoted) {
        return Cow::Borrowed(contents);
    }
    Cow::Owned(
        text.replace(&quoted, &format!("\"{module}/{target}/"))
            .into_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(module: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            format!("module {module}\n\ngo 1.25\n"),
        )
        .unwrap();
        dir
    }

    #[test]
    fn reads_the_module_line() {
        assert_eq!(
            module_of("// x\nmodule example.com/app\n"),
            Some("example.com/app")
        );
        assert_eq!(module_of("module \"app\"\n"), Some("app"));
        assert_eq!(module_of("modules x\n"), None);
    }

    #[test]
    fn points_registry_imports_at_the_target() {
        let dir = project("example.com/app");
        let source = b"import (\n\t\"context\"\n\n\t\"go.chanx.dev/kit/presence\"\n)\n";

        let localized = localize(
            source,
            "store.go",
            Some("go.chanx.dev/kit"),
            "./internal/wskits/",
            dir.path(),
        );

        assert_eq!(
            std::str::from_utf8(&localized).unwrap(),
            "import (\n\t\"context\"\n\n\t\"example.com/app/internal/wskits/presence\"\n)\n"
        );
    }

    #[test]
    fn leaves_other_files_and_modules_alone() {
        let dir = project("app");
        let source = b"\"go.chanx.dev/kit/presence\"";

        assert!(matches!(
            localize(
                source,
                "README.md",
                Some("go.chanx.dev/kit"),
                "x",
                dir.path()
            ),
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            localize(
                b"\"go.chanx.dev/kitchen/x\"",
                "a.go",
                Some("go.chanx.dev/kit"),
                "x",
                dir.path()
            ),
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            localize(source, "a.go", None, "x", dir.path()),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn needs_a_go_mod() {
        let dir = tempfile::tempdir().unwrap();

        let localized = localize(
            b"\"go.chanx.dev/kit/x\"",
            "a.go",
            Some("go.chanx.dev/kit"),
            "t",
            dir.path(),
        );

        assert!(matches!(localized, Cow::Borrowed(_)));
    }
}
