mod app;
mod browser;
mod clipboard;
mod config;
mod domain;
mod error;
mod fs;
mod github;
mod keymap;
mod parse;
mod render;
#[cfg(test)]
mod test_fixtures;

pub use app::App;
pub use domain::Document;
pub use error::AppError;
pub use github::{
    GitHubAuth, GitHubUrl, PR_LISTING_FLAG, build_pr_listing_markdown, fetch_blob_content,
    fetch_pr_info, parse_github_url, pr_listing_opt_in_required_message, resolve_auth,
    rewrite_relative_links, take_pr_listing_flag,
};
pub use parse::{MarkupFormat, parse_document, parse_with_path};
