//! Preview open timing and popup buffer warming.

use crate::domain::{LinkId, LinkKind, PreviewLoadStatus};

use super::App;

pub(crate) const PREVIEW_ZOOM_MIN: f32 = 0.25;
pub(crate) const PREVIEW_ZOOM_MAX: f32 = 4.0;
pub(crate) const PREVIEW_ZOOM_STEP: f32 = 1.15;

impl App {
    pub(crate) fn preview_load_status(&self, link_id: LinkId) -> PreviewLoadStatus {
        let Some(link) = self.document.links.get(link_id.0) else {
            return PreviewLoadStatus::Idle;
        };
        match link.kind {
            LinkKind::Mermaid => self.mermaid_render.preview_status(link_id, &self.rendered),
            LinkKind::Image => {
                self.image_render
                    .preview_status(link_id, &self.document, &self.rendered)
            }
            LinkKind::Toc => PreviewLoadStatus::Ready,
            _ => PreviewLoadStatus::Idle,
        }
    }

    pub(crate) fn preview_ready_to_open(&self, link_id: LinkId) -> bool {
        let Some(link) = self.document.links.get(link_id.0) else {
            return false;
        };
        if link.kind == LinkKind::Toc {
            return true;
        }
        if self.preview_protocol_cached(link_id) {
            return true;
        }
        matches!(self.preview_load_status(link_id), PreviewLoadStatus::Failed)
    }

    fn preview_protocol_cached(&self, link_id: LinkId) -> bool {
        let Some(link) = self.document.links.get(link_id.0) else {
            return false;
        };
        self.rendered
            .preview_protocol(link_id.0, link.kind, link.url.as_str())
            .is_some()
    }

    pub(crate) fn maybe_warm_selected_preview(&mut self) {
        let Some(link_id) = self.view_state.selected_link() else {
            return;
        };
        let Some(link) = self.document.links.get(link_id.0) else {
            return;
        };
        if !link.kind.is_preview() {
            return;
        }
        if self.preview_protocol_cached(link_id) {
            self.warm_preview_cache(link_id);
        }
    }

    pub(crate) fn warm_preview_cache(&mut self, link_id: LinkId) {
        let Some(link) = self.document.links.get(link_id.0).cloned() else {
            return;
        };
        let Some(protocol) =
            self.rendered
                .preview_protocol(link_id.0, link.kind, link.url.as_str())
        else {
            return;
        };
        let title = link
            .title
            .as_deref()
            .unwrap_or(link.url.as_str())
            .to_string();
        let terminal = self.view_state.terminal_size();
        self.preview_render_cache
            .ensure(link_id, terminal, &title, protocol);
    }

    pub(crate) fn open_preview_now(&mut self, link_id: LinkId) {
        self.reset_preview_zoom();
        self.warm_preview_cache(link_id);
        self.view_state = self.view_state.clone().open_preview(link_id);
    }

    pub(crate) fn try_complete_pending_preview(&mut self) -> bool {
        let Some(link_id) = self.pending_preview else {
            return false;
        };
        if !self.preview_ready_to_open(link_id) {
            return false;
        }
        self.pending_preview = None;
        self.open_preview_now(link_id);
        true
    }

    pub(crate) fn invalidate_preview_caches(&mut self) {
        self.preview_render_cache.clear();
        self.rendered.mermaid_images.clear();
        self.rendered.markdown_images.clear();
        self.mermaid_render.begin_document();
        self.image_render.begin_document();
        self.document_prefetch.begin_document();
        self.invalidate_prefetch_viewport();
        self.maybe_prefetch_visible_links();
    }

    pub(crate) fn adjust_preview_zoom(&mut self, factor: f32) {
        if self.view_state.mode().preview_link().is_none() {
            return;
        }
        let next = (self.preview_zoom * factor).clamp(PREVIEW_ZOOM_MIN, PREVIEW_ZOOM_MAX);
        if (next - self.preview_zoom).abs() < f32::EPSILON {
            return;
        }
        self.preview_zoom = next;
    }

    pub(crate) fn reset_preview_zoom(&mut self) {
        self.preview_zoom = 1.0;
    }
}

pub(crate) fn preview_waiting_message(kind: LinkKind) -> String {
    match kind {
        LinkKind::Mermaid => "Rendering mermaid diagram…".to_string(),
        LinkKind::Image => "Loading image…".to_string(),
        _ => "Loading preview…".to_string(),
    }
}

pub(crate) fn preview_failed_message(kind: LinkKind) -> String {
    match kind {
        LinkKind::Mermaid => "[failed to render mermaid diagram]".to_string(),
        LinkKind::Image => "[failed to load image]".to_string(),
        _ => "[failed to load preview]".to_string(),
    }
}

/// Shown instead of an inline image when the terminal's graphics protocol
/// falls back to Halfblocks and the diagram/image was opened externally.
pub(crate) fn preview_external_open_message() -> String {
    if cfg!(target_os = "macos") {
        "Opened in Preview.app — press o/Esc to close.".to_string()
    } else {
        "Opened in external viewer — press o/Esc to close.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_open_message_mentions_close_keys() {
        let message = preview_external_open_message();
        assert!(message.contains("Esc"));
        assert!(message.contains("o/"));
    }
}
