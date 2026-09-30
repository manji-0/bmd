//! GitHub HTTP fetch and authentication.

use super::url::{GitHubBlobUrl, GitHubPrUrl, PrDocumentFile, PrInfo};

#[derive(Clone)]
pub enum GitHubAuth {
    Token(String),
    None,
}

impl std::fmt::Debug for GitHubAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.write_str("Token(***)"),
            Self::None => f.write_str("None"),
        }
    }
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

/// GET `url` with the optional `Accept` media type and bearer token.
fn get(
    agent: &ureq::Agent,
    url: &str,
    accept: Option<&str>,
    auth: &GitHubAuth,
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let mut req = agent.get(url);
    if let Some(accept) = accept {
        req = req.header("Accept", accept);
    }
    if let GitHubAuth::Token(token) = auth {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }
    req.call()
}

fn api_error(error: ureq::Error, body: impl FnOnce() -> String) -> GitHubError {
    match error {
        ureq::Error::StatusCode(status) => GitHubError::Api {
            status,
            body: body(),
        },
        other => GitHubError::Http(other.to_string()),
    }
}

/// Fetch raw content for a GitHub blob URL.
pub fn fetch_blob_content(blob: &GitHubBlobUrl, auth: &GitHubAuth) -> Result<String, GitHubError> {
    let agent = build_agent();
    // raw.githubusercontent.com serves public repos without auth; private repos
    // 404 there and fall back to the Contents API.
    let response = match get(&agent, &blob.raw_url(), None, auth) {
        Err(ureq::Error::StatusCode(404)) => {
            let api_url = format!(
                "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
                blob.owner, blob.repo, blob.path, blob.git_ref
            );
            get(
                &agent,
                &api_url,
                Some("application/vnd.github.raw+json"),
                auth,
            )
            .map_err(|e| api_error(e, || format!("contents API failed for {}", blob.path)))?
        }
        other => other.map_err(|e| GitHubError::Http(e.to_string()))?,
    };
    response
        .into_body()
        .read_to_string()
        .map_err(|e| GitHubError::Http(e.to_string()))
}

#[derive(serde::Deserialize)]
struct PrJson {
    title: Option<String>,
    head: BranchJson,
    base: BranchJson,
}

#[derive(serde::Deserialize)]
struct BranchJson {
    sha: Option<String>,
    #[serde(rename = "ref")]
    git_ref: Option<String>,
}

#[derive(serde::Deserialize)]
struct FileJson {
    #[serde(default)]
    filename: String,
    status: Option<String>,
}

const FILES_PER_PAGE: usize = 100;

/// Fetch PR metadata and document file list.
pub fn fetch_pr_info(pr: &GitHubPrUrl, auth: &GitHubAuth) -> Result<PrInfo, GitHubError> {
    const JSON: Option<&str> = Some("application/vnd.github.v3+json");
    let agent = build_agent();
    let base = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        pr.owner, pr.repo, pr.number
    );
    let pr_json: PrJson = get(&agent, &base, JSON, auth)
        .map_err(|e| api_error(e, || format!("PR #{} not found", pr.number)))?
        .into_body()
        .read_json()
        .map_err(|e| GitHubError::Parse(e.to_string()))?;

    let mut files = Vec::new();
    for page in 1.. {
        let url = format!("{base}/files?per_page={FILES_PER_PAGE}&page={page}");
        let page_files: Vec<FileJson> = get(&agent, &url, JSON, auth)
            .map_err(|e| GitHubError::Http(e.to_string()))?
            .into_body()
            .read_json()
            .map_err(|e| GitHubError::Parse(e.to_string()))?;
        let last_page = page_files.len() < FILES_PER_PAGE;
        files.extend(page_files);
        if last_page {
            break;
        }
    }
    pr_info(pr_json, files)
}

fn pr_info(pr: PrJson, files: Vec<FileJson>) -> Result<PrInfo, GitHubError> {
    Ok(PrInfo {
        title: pr.title.unwrap_or_else(|| "(untitled)".into()),
        head_sha: pr
            .head
            .sha
            .ok_or_else(|| GitHubError::Parse("missing head.sha".into()))?,
        base_ref: pr.base.git_ref.unwrap_or_else(|| "main".into()),
        head_ref: pr.head.git_ref.unwrap_or_else(|| "unknown".into()),
        files: files
            .into_iter()
            .filter(|file| !file.filename.is_empty())
            .map(|file| PrDocumentFile {
                filename: file.filename,
                status: file.status.unwrap_or_else(|| "modified".into()),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_info_maps_api_json_with_defaults() {
        let pr: PrJson = serde_json::from_str(
            r#"{"title":"Docs","head":{"sha":"abc","ref":"feat"},"base":{"ref":"dev"}}"#,
        )
        .unwrap();
        let files: Vec<FileJson> = serde_json::from_str(
            r#"[{"filename":"a.md","status":"added"},{"filename":"b.rst"},{"status":"removed"}]"#,
        )
        .unwrap();
        let info = pr_info(pr, files).unwrap();
        assert_eq!(
            (info.title.as_str(), info.head_sha.as_str()),
            ("Docs", "abc")
        );
        assert_eq!(
            (info.base_ref.as_str(), info.head_ref.as_str()),
            ("dev", "feat")
        );
        let files: Vec<_> = info
            .files
            .iter()
            .map(|f| (f.filename.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(files, [("a.md", "added"), ("b.rst", "modified")]);

        let bare: PrJson = serde_json::from_str(r#"{"head":{"sha":"x"},"base":{}}"#).unwrap();
        let info = pr_info(bare, vec![]).unwrap();
        assert_eq!(
            (
                info.title.as_str(),
                info.base_ref.as_str(),
                info.head_ref.as_str()
            ),
            ("(untitled)", "main", "unknown")
        );
        let no_sha: PrJson = serde_json::from_str(r#"{"head":{},"base":{}}"#).unwrap();
        assert!(matches!(
            pr_info(no_sha, vec![]),
            Err(GitHubError::Parse(_))
        ));
    }

    #[test]
    fn github_auth_debug_redacts_token() {
        let auth = GitHubAuth::Token("ghp_secret_value".into());
        let rendered = format!("{auth:?}");
        assert_eq!(rendered, "Token(***)");
        assert!(!rendered.contains("ghp_secret_value"));
    }
}
