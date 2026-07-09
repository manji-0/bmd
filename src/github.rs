//! GitHub URL parsing, content fetching, and link rewriting.

use std::path::Path;

use crate::domain::{Document, LinkKind, LinkUrl};
use crate::parse::MarkupFormat;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubBlobUrl {
    pub owner: String,
    pub repo: String,
    pub git_ref: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubPrUrl {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitHubUrl {
    Blob(GitHubBlobUrl),
    PullRequest(GitHubPrUrl),
}

#[derive(Clone, Debug)]
pub enum GitHubAuth {
    Token(String),
    None,
}

pub struct PrInfo {
    pub title: String,
    pub head_sha: String,
    pub base_ref: String,
    pub head_ref: String,
    pub files: Vec<PrDocumentFile>,
}

pub struct PrDocumentFile {
    pub filename: String,
    pub status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("GitHub API error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("failed to parse response: {0}")]
    Parse(String),
}

// ---------------------------------------------------------------------------
// URL parsing
// ---------------------------------------------------------------------------

/// Parse a URL into a structured GitHub URL, if it matches a known pattern.
pub fn parse_github_url(url: &str) -> Option<GitHubUrl> {
    let url = url.split('#').next().unwrap_or(url);
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;

    let segments: Vec<&str> = path.splitn(5, '/').collect();
    if segments.len() < 3 {
        return None;
    }

    let owner = segments[0];
    let repo = segments[1];
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    match segments.get(2).copied() {
        Some("blob") if segments.len() >= 5 => {
            let git_ref = segments[3];
            let file_path = segments[4].trim_end_matches('/');
            if git_ref.is_empty() || file_path.is_empty() {
                return None;
            }
            Some(GitHubUrl::Blob(GitHubBlobUrl {
                owner: owner.to_string(),
                repo: repo.to_string(),
                git_ref: git_ref.to_string(),
                path: file_path.to_string(),
            }))
        }
        Some("pull") if segments.len() >= 4 => {
            let num_str = segments[3].trim_end_matches('/');
            let number = num_str.parse::<u64>().ok()?;
            Some(GitHubUrl::PullRequest(GitHubPrUrl {
                owner: owner.to_string(),
                repo: repo.to_string(),
                number,
            }))
        }
        _ => None,
    }
}

/// Extract the `#fragment` from a URL, if present.
pub fn url_fragment(url: &str) -> Option<&str> {
    url.split_once('#').map(|(_, frag)| frag).filter(|f| !f.is_empty())
}

// ---------------------------------------------------------------------------
// Document extension check
// ---------------------------------------------------------------------------

/// Returns `true` when the file path has a supported document extension.
pub fn is_supported_document_extension(path: &str) -> bool {
    MarkupFormat::from_path(Path::new(path)).is_some()
}

// ---------------------------------------------------------------------------
// Relative path resolution
// ---------------------------------------------------------------------------

impl GitHubBlobUrl {
    /// Directory portion of the path (e.g. `"docs"` for `"docs/guide.md"`).
    fn directory(&self) -> &str {
        match self.path.rfind('/') {
            Some(pos) => &self.path[..pos],
            None => "",
        }
    }

    /// Resolve a relative path against this blob's directory.
    pub fn resolve_relative(&self, relative: &str) -> GitHubBlobUrl {
        let base_dir = self.directory();
        let resolved = resolve_path(base_dir, relative);
        GitHubBlobUrl {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            git_ref: self.git_ref.clone(),
            path: resolved,
        }
    }

    /// Construct the GitHub web URL for this blob.
    pub fn to_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/blob/{}/{}",
            self.owner, self.repo, self.git_ref, self.path
        )
    }

    /// Construct the raw content URL.
    pub fn raw_url(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.owner, self.repo, self.git_ref, self.path
        )
    }
}

/// Resolve `relative` against `base_dir`, collapsing `.` and `..` segments.
fn resolve_path(base_dir: &str, relative: &str) -> String {
    let mut parts: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for segment in relative.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

// ---------------------------------------------------------------------------
// Link rewriting
// ---------------------------------------------------------------------------

/// Rewrite relative `LinkKind::Document` links to absolute GitHub blob URLs.
pub fn rewrite_relative_links(document: &mut Document, blob: &GitHubBlobUrl) {
    for link in &mut document.links {
        if link.kind != LinkKind::Document {
            continue;
        }
        let url_str = link.url.as_str().to_string();
        let (path_part, fragment) = crate::domain::document_link_path_part(&url_str);
        if path_part.is_empty() {
            continue;
        }
        let resolved = blob.resolve_relative(path_part);
        let mut new_url = resolved.to_url();
        if let Some(frag) = fragment {
            new_url.push('#');
            new_url.push_str(frag);
        }
        if let Ok(new_link_url) = LinkUrl::new(new_url) {
            link.url = new_link_url;
            link.kind = LinkKind::Web;
        }
    }
}

// ---------------------------------------------------------------------------
// PR listing markdown generation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

/// Resolve GitHub authentication: try `gh auth token`, then `GITHUB_TOKEN` env.
pub fn resolve_auth() -> GitHubAuth {
    if let Ok(output) = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        && output.status.success()
    {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return GitHubAuth::Token(token);
        }
    }
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        return GitHubAuth::Token(token);
    }
    GitHubAuth::None
}

// ---------------------------------------------------------------------------
// HTTP fetching
// ---------------------------------------------------------------------------

fn build_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        // GitHub's hostnames are reliably reachable over IPv4. Networks with
        // broken/black-holed IPv6 routes otherwise burn most of the connect
        // budget on dead IPv6 addresses before falling back (ureq tries
        // resolved addresses sequentially, unlike curl's Happy Eyeballs).
        .ip_family(ureq::config::IpFamily::Ipv4Only)
        .build();
    ureq::Agent::new_with_config(config)
}

fn auth_header(auth: &GitHubAuth) -> Option<String> {
    match auth {
        GitHubAuth::Token(token) => Some(format!("Bearer {token}")),
        GitHubAuth::None => None,
    }
}

/// Fetch raw content for a GitHub blob URL.
pub fn fetch_blob_content(blob: &GitHubBlobUrl, auth: &GitHubAuth) -> Result<String, GitHubError> {
    let agent = build_agent();

    // Try raw.githubusercontent.com first (works for public repos without auth)
    let raw_url = blob.raw_url();
    let mut req = agent.get(&raw_url);
    if let Some(header) = auth_header(auth) {
        req = req.header("Authorization", &header);
    }
    match req.call() {
        Ok(response) => {
            return response
                .into_body()
                .read_to_string()
                .map_err(|e| GitHubError::Http(e.to_string()));
        }
        Err(ureq::Error::StatusCode(404)) => {
            // Fall through to API endpoint for private repos
        }
        Err(e) => return Err(GitHubError::Http(e.to_string())),
    }

    // Fallback: GitHub Contents API (requires auth for private repos)
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        blob.owner, blob.repo, blob.path, blob.git_ref
    );
    let mut req = agent
        .get(&api_url)
        .header("Accept", "application/vnd.github.raw+json");
    if let Some(header) = auth_header(auth) {
        req = req.header("Authorization", &header);
    }
    let response = req.call().map_err(|e| match e {
        ureq::Error::StatusCode(status) => GitHubError::Api {
            status,
            body: format!("contents API failed for {}", blob.path),
        },
        other => GitHubError::Http(other.to_string()),
    })?;

    response
        .into_body()
        .read_to_string()
        .map_err(|e| GitHubError::Http(e.to_string()))
}

/// Fetch PR metadata and document file list.
pub fn fetch_pr_info(pr: &GitHubPrUrl, auth: &GitHubAuth) -> Result<PrInfo, GitHubError> {
    let agent = build_agent();

    // Fetch PR details
    let pr_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        pr.owner, pr.repo, pr.number
    );
    let mut req = agent
        .get(&pr_url)
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(header) = auth_header(auth) {
        req = req.header("Authorization", &header);
    }
    let pr_response = req.call().map_err(|e| match e {
        ureq::Error::StatusCode(status) => GitHubError::Api {
            status,
            body: format!("PR #{} not found", pr.number),
        },
        other => GitHubError::Http(other.to_string()),
    })?;

    let pr_json: serde_json::Value = pr_response
        .into_body()
        .read_json::<serde_json::Value>()
        .map_err(|e| GitHubError::Parse(e.to_string()))?;

    let title = pr_json["title"]
        .as_str()
        .unwrap_or("(untitled)")
        .to_string();
    let head_sha = pr_json["head"]["sha"]
        .as_str()
        .ok_or_else(|| GitHubError::Parse("missing head.sha".into()))?
        .to_string();
    let base_ref = pr_json["base"]["ref"]
        .as_str()
        .unwrap_or("main")
        .to_string();
    let head_ref = pr_json["head"]["ref"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    // Fetch changed files (paginated)
    let mut files = Vec::new();
    let mut page = 1u32;
    loop {
        let files_url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/files?per_page=100&page={}",
            pr.owner, pr.repo, pr.number, page
        );
        let mut req = agent
            .get(&files_url)
            .header("Accept", "application/vnd.github.v3+json");
        if let Some(header) = auth_header(auth) {
            req = req.header("Authorization", &header);
        }
        let files_response = req.call().map_err(|e| GitHubError::Http(e.to_string()))?;

        let page_files: Vec<serde_json::Value> = files_response
            .into_body()
            .read_json::<Vec<serde_json::Value>>()
            .map_err(|e| GitHubError::Parse(e.to_string()))?;

        if page_files.is_empty() {
            break;
        }

        for file in &page_files {
            let filename = file["filename"].as_str().unwrap_or_default().to_string();
            let status = file["status"].as_str().unwrap_or("modified").to_string();
            if !filename.is_empty() {
                files.push(PrDocumentFile { filename, status });
            }
        }

        if page_files.len() < 100 {
            break;
        }
        page += 1;
    }

    Ok(PrInfo {
        title,
        head_sha,
        base_ref,
        head_ref,
        files,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blob_url() {
        let url = "https://github.com/owner/repo/blob/main/docs/guide.md";
        let result = parse_github_url(url);
        assert_eq!(
            result,
            Some(GitHubUrl::Blob(GitHubBlobUrl {
                owner: "owner".into(),
                repo: "repo".into(),
                git_ref: "main".into(),
                path: "docs/guide.md".into(),
            }))
        );
    }

    #[test]
    fn parses_blob_url_with_commit_sha() {
        let url = "https://github.com/kkhs/platform-domain-app/blob/29839af7f5c8c24a4b355d07e529f4e01fc262d3/docs/ciam/adr/adr-file.md";
        let result = parse_github_url(url);
        assert!(matches!(result, Some(GitHubUrl::Blob(ref b)) if b.git_ref == "29839af7f5c8c24a4b355d07e529f4e01fc262d3"));
    }

    #[test]
    fn parses_blob_url_with_fragment() {
        let url = "https://github.com/owner/repo/blob/main/README.md#section";
        let result = parse_github_url(url);
        assert_eq!(
            result,
            Some(GitHubUrl::Blob(GitHubBlobUrl {
                owner: "owner".into(),
                repo: "repo".into(),
                git_ref: "main".into(),
                path: "README.md".into(),
            }))
        );
        assert_eq!(url_fragment(url), Some("section"));
    }

    #[test]
    fn parses_pr_url() {
        let url = "https://github.com/kkhs/platform-domain-app/pull/6988";
        let result = parse_github_url(url);
        assert_eq!(
            result,
            Some(GitHubUrl::PullRequest(GitHubPrUrl {
                owner: "kkhs".into(),
                repo: "platform-domain-app".into(),
                number: 6988,
            }))
        );
    }

    #[test]
    fn rejects_non_github_url() {
        assert_eq!(parse_github_url("https://example.com/foo"), None);
        assert_eq!(parse_github_url("not-a-url"), None);
    }

    #[test]
    fn rejects_incomplete_github_url() {
        assert_eq!(parse_github_url("https://github.com/owner"), None);
        assert_eq!(parse_github_url("https://github.com/owner/repo"), None);
        assert_eq!(
            parse_github_url("https://github.com/owner/repo/blob"),
            None
        );
        assert_eq!(
            parse_github_url("https://github.com/owner/repo/blob/main"),
            None
        );
    }

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
    fn resolve_relative_simple() {
        let blob = GitHubBlobUrl {
            owner: "o".into(),
            repo: "r".into(),
            git_ref: "main".into(),
            path: "docs/guide.md".into(),
        };
        let resolved = blob.resolve_relative("./other.md");
        assert_eq!(resolved.path, "docs/other.md");
    }

    #[test]
    fn resolve_relative_parent() {
        let blob = GitHubBlobUrl {
            owner: "o".into(),
            repo: "r".into(),
            git_ref: "main".into(),
            path: "docs/sub/guide.md".into(),
        };
        let resolved = blob.resolve_relative("../README.md");
        assert_eq!(resolved.path, "docs/README.md");
    }

    #[test]
    fn resolve_relative_root_level() {
        let blob = GitHubBlobUrl {
            owner: "o".into(),
            repo: "r".into(),
            git_ref: "main".into(),
            path: "docs/guide.md".into(),
        };
        let resolved = blob.resolve_relative("../README.md");
        assert_eq!(resolved.path, "README.md");
    }

    #[test]
    fn blob_to_url() {
        let blob = GitHubBlobUrl {
            owner: "owner".into(),
            repo: "repo".into(),
            git_ref: "main".into(),
            path: "docs/guide.md".into(),
        };
        assert_eq!(
            blob.to_url(),
            "https://github.com/owner/repo/blob/main/docs/guide.md"
        );
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
