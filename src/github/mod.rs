//! GitHub URL parsing, content fetching, and link rewriting.

mod fetch;
mod listing;
mod rewrite;
mod url;

pub use fetch::{GitHubAuth, GitHubError, fetch_blob_content, fetch_pr_info, resolve_auth};
pub use listing::{
    PR_LISTING_FLAG, build_pr_listing_markdown, is_supported_document_extension,
    pr_listing_opt_in_required_message, take_pr_listing_flag,
};
pub use rewrite::rewrite_relative_links;
pub use url::{
    GitHubBlobUrl, GitHubPrUrl, GitHubUrl, PrDocumentFile, PrInfo, parse_github_url, url_fragment,
};
