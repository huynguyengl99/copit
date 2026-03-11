mod add;
mod init;
mod licenses_sync;
mod remove;
mod update;
mod update_all;

use assert_cmd::Command;
use std::io::Write;

/// Build a `copit` CLI command for testing.
pub fn copit_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("copit")
}

/// Create an in-memory ZIP archive with the given files.
pub fn create_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, contents) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(contents).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

use predicates::prelude::*;

#[test]
fn no_args_shows_help() {
    copit_cmd()
        .assert()
        .failure()
        .stderr(predicates::str::contains("Usage"));
}

#[test]
fn help_shows_all_commands() {
    copit_cmd().arg("--help").assert().success().stdout(
        predicates::str::contains("init")
            .and(predicates::str::contains("add"))
            .and(predicates::str::contains("remove"))
            .and(predicates::str::contains("update"))
            .and(predicates::str::contains("update-all"))
            .and(predicates::str::contains("licenses-sync")),
    );
}
