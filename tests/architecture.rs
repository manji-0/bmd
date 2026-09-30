//! Layer-boundary checks for the rules in PLAN.md (層ルール). These run as part of `cargo test`.

use std::fs;
use std::path::{Path, PathBuf};

const MODULES: &[&str] = &[
    "app",
    "browser",
    "clipboard",
    "config",
    "domain",
    "error",
    "fs",
    "github",
    "keymap",
    "parse",
    "render",
];

/// `(source path, crate modules it may reference besides itself)`.
const LAYERS: &[(&str, &[&str])] = &[
    ("src/domain", &[]),
    ("src/error.rs", &["domain"]),
    ("src/parse", &["domain", "error"]),
    ("src/render", &["domain", "error"]),
    ("src/keymap.rs", &["domain", "error"]),
    ("src/config.rs", &["keymap", "render", "error"]),
    ("src/fs.rs", &["domain"]),
    ("src/browser.rs", &["domain", "error"]),
    ("src/clipboard.rs", &["error"]),
    ("src/github/url.rs", &[]),
    ("src/github", &["domain", "parse", "error"]),
];

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path.to_path_buf());
        return;
    }
    for entry in fs::read_dir(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() || path.extension().is_some_and(|ext| ext == "rs") {
            rust_files(&path, out);
        }
    }
}

/// Source with unit-test modules stripped; tests may reach across layers.
fn production_source(path: &Path) -> String {
    if path.file_name().is_some_and(|name| name == "tests.rs") {
        return String::new();
    }
    let src = fs::read_to_string(path).unwrap();
    src.split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap_or(&src)
        .to_string()
}

fn assert_absent(path: &Path, src: &str, needles: &[String]) {
    for needle in needles {
        assert!(
            !src.contains(needle.as_str()),
            "{} must not reference `{needle}`",
            path.display()
        );
    }
}

#[test]
fn modules_only_depend_on_allowed_layers() {
    for (root, allowed) in LAYERS {
        let own = root
            .trim_start_matches("src/")
            .trim_end_matches(".rs")
            .split('/')
            .next()
            .unwrap();
        let forbidden: Vec<String> = MODULES
            .iter()
            .filter(|module| **module != own && !allowed.contains(module))
            .map(|module| format!("crate::{module}"))
            .collect();
        let mut files = Vec::new();
        rust_files(Path::new(root), &mut files);
        for path in files {
            assert_absent(&path, &production_source(&path), &forbidden);
        }
    }
}

#[test]
fn pure_layers_have_no_io() {
    let needles = ["use std::fs", "std::fs::", "std::process", "ureq"].map(String::from);
    for root in ["src/domain", "src/github/url.rs"] {
        let mut files = Vec::new();
        rust_files(Path::new(root), &mut files);
        for path in files {
            assert_absent(&path, &production_source(&path), &needles);
        }
    }
    let mut files = Vec::new();
    rust_files(Path::new("src/render"), &mut files);
    for path in files {
        assert_absent(
            &path,
            &production_source(&path),
            &["process::Command".into()],
        );
    }
}
