//! Layer-boundary checks. These run as part of `cargo test` / CI.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files);
    files.sort();
    files
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn production_source(src: &str) -> &str {
    src.split("#[cfg(test)]").next().unwrap_or(src)
}

fn assert_no_needles(path: &Path, src: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            !src.contains(needle),
            "{} must not contain `{needle}`",
            path.display()
        );
    }
}

#[test]
fn domain_does_not_depend_on_outer_layers_or_fs() {
    let needles = [
        "use crate::app",
        "use crate::parse",
        "use crate::render",
        "use crate::github",
        "use crate::config",
        "use crate::keymap",
        "use crate::browser",
        "use crate::clipboard",
        "use crate::fs",
        "use std::fs",
        "std::fs::",
        "std::process::",
    ];
    for path in rust_files(Path::new("src/domain")) {
        let src = fs::read_to_string(&path).unwrap();
        assert_no_needles(&path, production_source(&src), &needles);
    }
}

#[test]
fn keymap_does_not_import_config() {
    let src = fs::read_to_string("src/keymap.rs").unwrap();
    assert_no_needles(
        Path::new("src/keymap.rs"),
        production_source(&src),
        &["use crate::config"],
    );
}

#[test]
fn render_production_does_not_import_parse() {
    for path in rust_files(Path::new("src/render")) {
        if path.file_name().is_some_and(|name| name == "tests.rs") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        assert_no_needles(
            &path,
            production_source(&src),
            &["use crate::parse", "crate::parse::"],
        );
    }
}

#[test]
fn github_url_module_has_no_http() {
    let src = fs::read_to_string("src/github/url.rs").unwrap();
    assert_no_needles(
        Path::new("src/github/url.rs"),
        production_source(&src),
        &["ureq", "std::process", "use crate::parse"],
    );
}
