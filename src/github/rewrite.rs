//! Rewrite relative document links to absolute GitHub blob URLs.

use crate::domain::{Document, Link, LinkKind, LinkUrl};

use super::url::GitHubBlobUrl;

/// Rewrite relative `LinkKind::Document` links to absolute GitHub blob URLs.
pub fn rewrite_relative_links(document: &mut Document, blob: &GitHubBlobUrl) {
    document.rewrite_document_links(|link| {
        if link.kind != LinkKind::Document {
            return None;
        }
        let url_str = link.url.as_str().to_string();
        let (path_part, fragment) = crate::domain::document_link_path_part(&url_str);
        if path_part.is_empty() {
            return None;
        }
        let resolved = blob.resolve_relative(path_part);
        let mut new_url = resolved.to_url();
        if let Some(frag) = fragment {
            new_url.push('#');
            new_url.push_str(frag);
        }
        let Ok(new_link_url) = LinkUrl::new(new_url) else {
            return None;
        };
        Some(Link {
            url: new_link_url,
            title: link.title.clone(),
            kind: LinkKind::Web,
        })
    });
}
