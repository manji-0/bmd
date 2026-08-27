//! PR listing markdown generated from GitHub metadata.

use std::path::Path;

use crate::parse::MarkupFormat;

use super::url::{GitHubPrUrl, PrDocumentFile, PrInfo};

/// Returns `true` when the file path has a supported document extension.
pub fn is_supported_document_extension(path: &str) -> bool {
    MarkupFormat::from_path(Path::new(path)).is_some()
}

/// Build a Markdown document listing the changed document files in a PR.
pub fn build_pr_listing_markdown(pr: &GitHubPrUrl, info: &PrInfo) -> String {
    let mut md = String::new();

    md.push_str(&format!("# PR #{}: {}\n\n", pr.number, info.title));
    md.push_str(&format!(
        "**base** `{}` ← **head** `{}`\n\n",
        info.base_ref, info.head_ref
    ));

    let doc_files: Vec<&PrDocumentFile> = info
        .files
        .iter()
        .filter(|f| is_supported_document_extension(&f.filename))
        .collect();

    if doc_files.is_empty() {
        md.push_str("*No document files changed in this PR.*\n");
        return md;
    }

    md.push_str(&format!(
        "**{} document file(s) changed**\n\n",
        doc_files.len()
    ));
    md.push_str("## Changed Documents\n\n");

    for file in &doc_files {
        let blob_url = format!(
            "https://github.com/{}/{}/blob/{}/{}",
            pr.owner, pr.repo, info.head_sha, file.filename
        );
        let status_label = match file.status.as_str() {
            "added" => " *(added)*",
            "removed" => " *(removed)*",
            "renamed" => " *(renamed)*",
            _ => "",
        };
        md.push_str(&format!(
            "- [{}]({}){}\n",
            file.filename, blob_url, status_label
        ));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_extensions() {
        assert!(is_supported_document_extension("docs/guide.md"));
        assert!(is_supported_document_extension("readme.markdown"));
        assert!(is_supported_document_extension("notes.rst"));
        assert!(is_supported_document_extension("book.adoc"));
        assert!(is_supported_document_extension("book.asciidoc"));
        assert!(!is_supported_document_extension("main.rs"));
        assert!(!is_supported_document_extension("config.json"));
        assert!(!is_supported_document_extension("no-extension"));
    }

    #[test]
    fn pr_listing_filters_and_formats() {
        let pr = GitHubPrUrl {
            owner: "o".into(),
            repo: "r".into(),
            number: 42,
        };
        let info = PrInfo {
            title: "Add docs".into(),
            head_sha: "abc123".into(),
            base_ref: "main".into(),
            head_ref: "feature".into(),
            files: vec![
                PrDocumentFile {
                    filename: "docs/guide.md".into(),
                    status: "added".into(),
                },
                PrDocumentFile {
                    filename: "src/main.rs".into(),
                    status: "modified".into(),
                },
                PrDocumentFile {
                    filename: "notes.rst".into(),
                    status: "modified".into(),
                },
            ],
        };
        let md = build_pr_listing_markdown(&pr, &info);
        assert!(md.contains("# PR #42: Add docs"));
        assert!(md.contains("docs/guide.md"));
        assert!(md.contains("notes.rst"));
        assert!(!md.contains("src/main.rs"));
        assert!(md.contains("2 document file(s) changed"));
        assert!(md.contains("*(added)*"));
    }

    #[test]
    fn pr_listing_empty_docs() {
        let pr = GitHubPrUrl {
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
        };
        let info = PrInfo {
            title: "Code only".into(),
            head_sha: "abc".into(),
            base_ref: "main".into(),
            head_ref: "fix".into(),
            files: vec![PrDocumentFile {
                filename: "src/lib.rs".into(),
                status: "modified".into(),
            }],
        };
        let md = build_pr_listing_markdown(&pr, &info);
        assert!(md.contains("No document files changed"));
    }
}
