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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_auth_debug_redacts_token() {
        let auth = GitHubAuth::Token("ghp_secret_value".into());
        let rendered = format!("{auth:?}");
        assert_eq!(rendered, "Token(***)");
        assert!(!rendered.contains("ghp_secret_value"));
    }
}
