//! GitHub URL parsing, content fetching, and link rewriting.

mod fetch;
mod listing;
mod rewrite;
mod url;

pub use fetch::{GitHubAuth, GitHubError, fetch_blob_content, fetch_pr_info, resolve_auth};
pub use listing::{build_pr_listing_markdown, is_supported_document_extension};
pub use rewrite::rewrite_relative_links;
pub use url::{
    GitHubBlobUrl, GitHubPrUrl, GitHubUrl, PrDocumentFile, PrInfo, parse_github_url, url_fragment,
};
