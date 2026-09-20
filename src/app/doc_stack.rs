//! Stack of previously viewed documents for in-app file navigation.

use std::path::PathBuf;

use crate::domain::{
    ChecklistState, DOCUMENT_STACK_MAX_LAYERS, Document, DocumentPrefetchSessionSnapshot,
    DocumentStackFull, ImageSessionSnapshot, LinkId, LinkJumpStack, LinkJumpStackFull, Marks,
    MermaidSessionSnapshot, NavStack, PriorAtLinkJump, TextSelection, ViewState,
};
use crate::render::{DocumentRenderCache, PreviewRenderCache, RenderedDocument};

use super::outline::OutlineUi;
use super::reload::FileWatch;

/// Full viewing state fixed at a document link jump, including render caches.
///
/// The live current document is not stored here. [`RenderedDocument`] holds loaded
/// image protocols; [`DocumentRenderCache`] and [`PreviewRenderCache`] preserve
/// warmed draw buffers for instant restore.
#[derive(Clone)]
pub(crate) struct DocumentFrame {
    pub document: Document,
    pub rendered: RenderedDocument,
    pub mermaid_session: MermaidSessionSnapshot,
    pub image_session: ImageSessionSnapshot,
    pub document_prefetch_session: DocumentPrefetchSessionSnapshot,
    pub document_cache: DocumentRenderCache,
    pub preview_render_cache: PreviewRenderCache,
    pub pending_preview: Option<LinkId>,
    pub preview_zoom: f32,
    pub toc_selected: usize,
    pub view_state: ViewState,
    pub scroll_visual: f32,
    pub scroll_anim_speed: f32,
    pub checklist_state: ChecklistState,
    pub source_label: Option<String>,
    pub base_path: Option<PathBuf>,
    pub file_watch: Option<FileWatch>,
    pub nav_stack: NavStack,
    pub marks: Marks,
    pub outline: OutlineUi,
    pub text_selection: Option<TextSelection>,
}

/// Document state fixed at the moment before a document link jump.
pub(crate) type FixedDocumentPrior = PriorAtLinkJump<DocumentFrame>;

/// Document navigation stack: the live current file lives in app state.
///
/// [`FixedDocumentPrior`] snapshots are stored only when following document links
/// via [`DocStack::fix_prior_on_link_jump`].
pub(crate) struct DocStack {
    stack: LinkJumpStack<DocumentFrame>,
}

impl Default for DocStack {
    fn default() -> Self {
        Self {
            stack: LinkJumpStack::with_max_layers(DOCUMENT_STACK_MAX_LAYERS),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl DocStack {
    pub fn max_layers() -> usize {
        DOCUMENT_STACK_MAX_LAYERS
    }

    pub fn max_frames() -> usize {
        DOCUMENT_STACK_MAX_LAYERS - 1
    }

    pub fn current_layer(&self) -> usize {
        self.stack.current_layer()
    }

    /// Fix the current document and store it before following a document link.
    ///
    /// On overflow the prior frame is returned so worker sessions consumed
    /// into it can be restored onto the live pools.
    pub fn fix_prior_on_link_jump(
        &mut self,
        prior: FixedDocumentPrior,
    ) -> Result<(), (DocumentStackFull, Box<DocumentFrame>)> {
        self.stack
            .fix_prior_on_link_jump(prior)
            .map_err(|(LinkJumpStackFull, prior)| (DocumentStackFull, Box::new(prior.into_inner())))
    }

    pub fn pop(&mut self) -> Option<DocumentFrame> {
        self.stack.restore_latest_prior().ok()
    }

    /// Take every stored document frame, oldest (root) first.
    pub fn take_all_frames(&mut self) -> Vec<DocumentFrame> {
        self.stack.take_all_priors()
    }

    /// Put frames back after a failed restore, oldest first.
    pub fn restore_frames(&mut self, frames: Vec<DocumentFrame>) {
        self.stack.restore_priors(frames);
    }

    pub fn len_frames(&self) -> usize {
        self.stack.fixed_prior_count()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_at_origin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stack_counts_current_file_as_layer_one() {
        let stack = DocStack::default();
        assert_eq!(DocStack::max_layers(), DOCUMENT_STACK_MAX_LAYERS);
        assert_eq!(DocStack::max_frames(), DOCUMENT_STACK_MAX_LAYERS - 1);
        assert_eq!(stack.current_layer(), 1);
        assert!(stack.is_empty());
    }
}
