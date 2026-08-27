//! GitHub URL types and pure path resolution.

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
    url.split_once('#')
        .map(|(_, frag)| frag)
        .filter(|f| !f.is_empty())
}

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
        assert!(
            matches!(result, Some(GitHubUrl::Blob(ref b)) if b.git_ref == "29839af7f5c8c24a4b355d07e529f4e01fc262d3")
        );
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
        assert_eq!(parse_github_url("https://github.com/owner/repo/blob"), None);
        assert_eq!(
            parse_github_url("https://github.com/owner/repo/blob/main"),
            None
        );
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
}
