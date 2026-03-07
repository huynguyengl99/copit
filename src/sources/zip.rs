//! ZIP archive extraction.
//!
//! Provides [`extract_from_bytes`] for extracting files from in-memory ZIP
//! archives, with support for path filtering and prefix stripping (used to
//! handle GitHub's archive format which wraps files in a `repo-ref/` directory).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// Extracted files: relative path to file contents.
pub type ExtractedFiles = HashMap<String, Vec<u8>>;

/// Extract files from a ZIP archive in memory.
///
/// If `inner_path` is provided, only files matching that prefix are extracted.
/// The `strip_prefix` is removed from the beginning of each path in the archive
/// (used to strip GitHub's top-level directory like `repo-ref/`).
pub fn extract_from_bytes(
    bytes: &[u8],
    inner_path: Option<&str>,
    strip_prefix: Option<&str>,
) -> Result<ExtractedFiles> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

    let mut files = HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Failed to read ZIP entry")?;

        if entry.is_dir() {
            continue;
        }

        let raw_name = entry.name().to_string();

        // Skip entries with path traversal components
        if raw_name.split('/').any(|c| c == "..") {
            continue;
        }

        // Strip prefix if provided
        let name = if let Some(prefix) = strip_prefix {
            match raw_name.strip_prefix(prefix) {
                Some(stripped) => stripped.to_string(),
                None => continue,
            }
        } else {
            raw_name
        };

        // Filter by inner_path if provided
        if let Some(inner) = inner_path {
            // Match exact file or directory prefix
            if name != inner && !name.starts_with(&format!("{inner}/")) {
                continue;
            }
        }

        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .with_context(|| format!("Failed to read ZIP entry: {name}"))?;

        files.insert(name, contents);
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn extract_no_filter() {
        let data = create_zip(&[("a.txt", b"aaa"), ("sub/b.txt", b"bbb")]);
        let files = extract_from_bytes(&data, None, None).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files["a.txt"], b"aaa");
        assert_eq!(files["sub/b.txt"], b"bbb");
    }

    #[test]
    fn extract_inner_path_file() {
        let data = create_zip(&[("a.txt", b"aaa"), ("sub/b.txt", b"bbb")]);
        let files = extract_from_bytes(&data, Some("sub/b.txt"), None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files["sub/b.txt"], b"bbb");
    }

    #[test]
    fn extract_inner_path_dir() {
        let data = create_zip(&[
            ("root/a.txt", b"aaa"),
            ("root/sub/b.txt", b"bbb"),
            ("other/c.txt", b"ccc"),
        ]);
        let files = extract_from_bytes(&data, Some("root"), None).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("root/a.txt"));
        assert!(files.contains_key("root/sub/b.txt"));
    }

    #[test]
    fn extract_strip_prefix() {
        let data = create_zip(&[
            ("repo-v1/src/lib.rs", b"code"),
            ("repo-v1/README.md", b"readme"),
        ]);
        let files = extract_from_bytes(&data, None, Some("repo-v1/")).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files["src/lib.rs"], b"code");
        assert_eq!(files["README.md"], b"readme");
    }

    #[test]
    fn extract_strip_prefix_and_inner_path() {
        let data = create_zip(&[
            ("repo-v1/src/lib.rs", b"code"),
            ("repo-v1/src/util.rs", b"util"),
            ("repo-v1/README.md", b"readme"),
        ]);
        let files = extract_from_bytes(&data, Some("src/lib.rs"), Some("repo-v1/")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files["src/lib.rs"], b"code");
    }

    #[test]
    fn extract_strip_prefix_no_match() {
        let data = create_zip(&[("repo-v1/src/lib.rs", b"code")]);
        let files = extract_from_bytes(&data, None, Some("other-prefix/")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn extract_inner_path_no_match() {
        let data = create_zip(&[("a.txt", b"aaa")]);
        let files = extract_from_bytes(&data, Some("nonexistent.txt"), None).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn extract_invalid_archive() {
        assert!(extract_from_bytes(b"not a zip", None, None).is_err());
    }

    #[test]
    fn extract_empty_archive() {
        let buf = std::io::Cursor::new(Vec::new());
        let zip = zip::ZipWriter::new(buf);
        let data = zip.finish().unwrap().into_inner();
        let files = extract_from_bytes(&data, None, None).unwrap();
        assert!(files.is_empty());
    }
}
