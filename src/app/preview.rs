//! Preview open timing and popup buffer warming.

use crate::domain::{LinkId, LinkKind, PreviewLoadStatus};

use super::App;

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
            _ => PreviewLoadStatus::Idle,
        }
    }

    pub(crate) fn preview_ready_to_open(&self, link_id: LinkId) -> bool {
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
