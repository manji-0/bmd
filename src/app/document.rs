//! In-app navigation to linked markdown files.

use std::path::PathBuf;

use crate::domain::{
    AnchorIdle, ChecklistState, ChecklistStyle, DocumentStackFull, document_link_path_part,
    document_stack_limit_message, normalize_document_path, plan_document_back, plan_document_reset,
    resolve_document_path,
};
use crate::error::AppError;
use crate::parse::parse_with_path;
use crate::render::{DocumentRenderCache, RenderedDocument};

use super::App;
use super::doc_stack::DocumentFrame;
use super::reload::FileWatch;
use super::scroll::SCROLL_ANIM_SPEED;

impl App {
    pub(crate) fn open_document_link(&mut self, dest: &str) {
        let resolved = match resolve_document_path(self.base_path.as_deref(), dest) {
            Ok(path) => normalize_document_path(path),
            Err(e) => {
                self.set_status_message(e.to_string());
                return;
            }
        };
        if !resolved.is_file() {
            self.set_status_message(format!("file not found: {}", resolved.display()));
            return;
        }

        let document = if let Some(document) = self.document_prefetch.ready_document(&resolved) {
            document
        } else {
            let content = match std::fs::read_to_string(&resolved) {
                Ok(content) => content,
                Err(e) => {
                    self.set_status_message(format!("read failed: {e}"));
                    return;
                }
            };
            match parse_with_path(Some(&resolved), &content) {
                Ok(document) => document,
                Err(e) => {
                    self.set_status_message(format!("parse error: {e}"));
                    return;
                }
            }
        };

        let anchor = document_link_path_part(dest).1;
        let prior = super::doc_stack::FixedDocumentPrior::fix(self.capture_document_frame());
        if let Err(DocumentStackFull) = self.doc_stack.fix_prior_on_link_jump(prior) {
            self.set_status_message(document_stack_limit_message());
            return;
        }

        if let Err(e) = self.apply_document(resolved, document) {
            self.set_status_message(e.to_string());
            // apply failed before mutating self; drop the pushed snapshot only.
            self.doc_stack.pop();
            return;
        }

        if let Some(anchor) = anchor {
            self.follow_anchor(anchor);
        }
    }

    pub(crate) fn doc_back(&mut self, idle: AnchorIdle) {
        if AnchorIdle::from_stack(&self.nav_stack) != Some(idle) {
            return;
        }
        let Some(()) = plan_document_back(idle, self.doc_stack.len_frames()) else {
            self.set_status_message("document stack empty".into());
            return;
        };
        let Some(frame) = self.doc_stack.pop() else {
            self.set_status_message("document stack empty".into());
            return;
        };
        match self.try_restore_document_frame(frame) {
            Ok(()) => {}
            Err(err) => {
                let (e, frame) = *err;
                self.set_status_message(e.to_string());
                self.doc_stack
                    .fix_prior_on_link_jump(super::doc_stack::FixedDocumentPrior::fix(frame))
                    .expect("restore rollback");
            }
        }
    }

    pub(crate) fn doc_reset(&mut self, idle: AnchorIdle) {
        if AnchorIdle::from_stack(&self.nav_stack) != Some(idle) {
            return;
        }
        let Some(()) = plan_document_reset(idle, self.doc_stack.len_frames()) else {
            return;
        };
        let Some(root) = self.doc_stack.take_root_prior() else {
            return;
        };
        match self.try_restore_document_frame(root) {
            Ok(()) => {}
            Err(err) => {
                let (e, frame) = *err;
                self.set_status_message(e.to_string());
                self.doc_stack
                    .fix_prior_on_link_jump(super::doc_stack::FixedDocumentPrior::fix(frame))
                    .expect("restore rollback");
            }
        }
    }

    fn capture_document_frame(&self) -> DocumentFrame {
        DocumentFrame {
            document: self.document.clone(),
            rendered: self.rendered.clone(),
            mermaid_session: self.mermaid_render.suspend(),
            image_session: self.image_render.suspend(),
            document_prefetch_session: self.document_prefetch.suspend(),
            document_cache: self.document_cache.clone(),
            preview_render_cache: self.preview_render_cache.clone(),
            pending_preview: self.pending_preview,
            view_state: self.view_state.clone(),
            scroll_visual: self.scroll_visual,
            scroll_anim_speed: self.scroll_anim_speed,
            tracked_scroll_position: self.tracked_scroll_position,
            show_terminal_images: self.show_terminal_images,
            checklist_state: self.checklist_state.clone(),
            source_label: self.source_label.clone(),
            base_path: self.base_path.clone(),
            file_watch: self.file_watch.clone(),
            nav_stack: self.nav_stack.clone(),
        }
    }

    fn apply_document(
        &mut self,
        path: PathBuf,
        document: crate::domain::Document,
    ) -> Result<(), AppError> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_apply_document) {
            return Err(AppError::TerminalImage("injected apply failure".into()));
        }
        let terminal_size = self.view_state.terminal_size();
        let rendered = RenderedDocument::new(&document, &self.picker, terminal_size, Some(&path))?;
        self.document = document;
        self.rendered = rendered;
        self.bump_document_revision();
        self.document_cache = DocumentRenderCache::default();
        self.preview_render_cache.clear();
        self.pending_preview = None;
        self.view_state = crate::domain::ViewState::new(terminal_size);
        self.nav_stack.clear();
        self.checklist_state = ChecklistState::new(ChecklistStyle::from_env());
        self.base_path = Some(path.clone());
        self.source_label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        self.file_watch = FileWatch::new(path).ok();
        self.scroll_visual = 0.0;
        self.scroll_anim_speed = SCROLL_ANIM_SPEED;
        self.tracked_scroll_position = 0.0;
        self.show_terminal_images = true;
        self.images_reenable_at = None;
        self.scroll_key_down_at = None;
        self.help_visible = false;
        self.mermaid_render.begin_document();
        self.image_render.begin_document();
        self.document_prefetch.begin_document();
        self.invalidate_prefetch_viewport();
        self.maybe_prefetch_visible_links();
        Ok(())
    }

    fn try_restore_document_frame(
        &mut self,
        frame: DocumentFrame,
    ) -> Result<(), Box<(AppError, DocumentFrame)>> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_document_restore) {
            return Err(Box::new((
                AppError::TerminalImage("injected restore failure".into()),
                frame,
            )));
        }
        self.document = frame.document;
        self.rendered = frame.rendered;
        self.bump_document_revision();
        self.view_state = frame.view_state;
        self.document_cache = frame.document_cache;
        self.preview_render_cache = frame.preview_render_cache;
        self.pending_preview = frame.pending_preview;
        self.scroll_visual = frame.scroll_visual;
        self.scroll_anim_speed = frame.scroll_anim_speed;
        self.tracked_scroll_position = frame.tracked_scroll_position;
        self.show_terminal_images = frame.show_terminal_images;
        self.checklist_state = frame.checklist_state;
        self.source_label = frame.source_label;
        self.base_path = frame.base_path;
        self.file_watch = frame.file_watch;
        self.nav_stack = frame.nav_stack;
        self.images_reenable_at = None;
        self.scroll_key_down_at = None;
        self.help_visible = false;
        let terminal_size = self.view_state.terminal_size();
        self.mermaid_render.resume(
            frame.mermaid_session,
            &self.document,
            &self.rendered,
            &self.picker,
            terminal_size,
        );
        self.image_render.resume(
            frame.image_session,
            &self.document,
            &self.rendered,
            self.base_path.as_ref(),
            &self.picker,
            terminal_size,
        );
        self.document_prefetch
            .resume(frame.document_prefetch_session);
        Ok(())
    }
}
