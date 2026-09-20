//! In-app navigation to linked markdown files.

use std::path::PathBuf;

use crate::domain::{
    AnchorIdle, ChecklistState, ChecklistStyle, DocumentPrefetchSessionSnapshot, DocumentStackFull,
    ImageSessionSnapshot, MermaidSessionSnapshot, document_link_path_part,
    document_stack_limit_message, resolve_document_path,
};
use crate::error::AppError;
use crate::fs::normalize_document_path;
use crate::parse::parse_with_path;
use crate::render::{DocumentRenderCache, RenderedDocument};

use super::App;
use super::doc_stack::DocumentFrame;
use super::reload::FileWatch;

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
        if self.push_document_prior().is_err() {
            self.set_status_message(document_stack_limit_message());
            return;
        }

        if let Err(e) = self.apply_document(resolved, document) {
            self.set_status_message(e.to_string());
            self.abort_document_jump();
            return;
        }

        if let Some(anchor) = anchor {
            self.follow_anchor(anchor);
        }
    }

    pub(crate) fn open_github_blob_link(
        &mut self,
        blob: &crate::github::GitHubBlobUrl,
        fragment: Option<&str>,
    ) {
        self.set_status_message(format!("fetching {}...", blob.path));

        let auth = self.ensure_github_auth();
        let content = match crate::github::fetch_blob_content(blob, &auth) {
            Ok(c) => c,
            Err(e) => {
                self.set_status_message(format!("fetch failed: {e}"));
                return;
            }
        };

        let format = crate::parse::MarkupFormat::from_path(std::path::Path::new(&blob.path))
            .unwrap_or(crate::parse::MarkupFormat::Markdown);

        let mut document = match crate::parse::parse_document(format, &content) {
            Ok(doc) => doc,
            Err(e) => {
                self.set_status_message(format!("parse error: {e}"));
                return;
            }
        };

        crate::github::rewrite_relative_links(&mut document, blob);

        if self.push_document_prior().is_err() {
            self.set_status_message(document_stack_limit_message());
            return;
        }

        if let Err(e) = self.apply_github_document(blob, document) {
            self.set_status_message(e.to_string());
            self.abort_document_jump();
            return;
        }

        if let Some(anchor) = fragment {
            self.follow_anchor(anchor);
        }
    }

    pub(crate) fn doc_back(&mut self, idle: AnchorIdle) {
        if AnchorIdle::from_stack(&self.nav_stack) != Some(idle) {
            return;
        }
        let Some(frame) = self.doc_stack.pop() else {
            self.set_status_message("document stack empty".into());
            return;
        };
        if let Err(err) = self.try_restore_document_frame(frame) {
            let (e, frame) = *err;
            self.set_status_message(e.to_string());
            self.doc_stack.restore_frames(vec![frame]);
        }
    }

    pub(crate) fn doc_reset(&mut self, idle: AnchorIdle) {
        if AnchorIdle::from_stack(&self.nav_stack) != Some(idle) {
            return;
        }
        let mut frames = self.doc_stack.take_all_frames().into_iter();
        let Some(root) = frames.next() else {
            return;
        };
        let rest: Vec<_> = frames.collect();
        if let Err(err) = self.try_restore_document_frame(root) {
            let (e, root) = *err;
            self.set_status_message(e.to_string());
            let mut frames = Vec::with_capacity(rest.len() + 1);
            frames.push(root);
            frames.extend(rest);
            self.doc_stack.restore_frames(frames);
        }
    }

    fn apply_github_document(
        &mut self,
        blob: &crate::github::GitHubBlobUrl,
        document: crate::domain::Document,
    ) -> Result<(), AppError> {
        let terminal_size = self.view_state.terminal_size();
        let rendered = RenderedDocument::new(&document, &self.picker, terminal_size, None)?;
        self.document = document;
        self.rendered = rendered;
        self.bump_document_revision();
        self.document_cache = DocumentRenderCache::default();
        self.preview.cache.clear();
        self.preview.pending = None;
        self.view_state = crate::domain::ViewState::new(terminal_size);
        self.nav_stack.clear();
        self.checklist_state = ChecklistState::new(ChecklistStyle::from_env());
        self.base_path = None;
        self.source_label = Some(blob.path.clone());
        self.file_watch = None;
        self.scroll.reset_to_top();
        self.help_visible = false;
        self.clear_marks();
        self.pending_input = super::pending::PendingInput::None;
        self.reset_transient_view_ui();
        self.mermaid_render.begin_document();
        self.image_render.begin_document();
        self.document_prefetch.begin_document();
        self.invalidate_prefetch_viewport();
        self.maybe_prefetch_visible_links();
        Ok(())
    }

    fn push_document_prior(&mut self) -> Result<(), DocumentStackFull> {
        let prior = super::doc_stack::FixedDocumentPrior::fix(self.capture_document_frame());
        match self.doc_stack.fix_prior_on_link_jump(prior) {
            Ok(()) => Ok(()),
            Err((DocumentStackFull, prior)) => {
                let prior = *prior;
                self.resume_worker_sessions(
                    prior.mermaid_session,
                    prior.image_session,
                    prior.document_prefetch_session,
                );
                Err(DocumentStackFull)
            }
        }
    }

    fn abort_document_jump(&mut self) {
        if let Some(frame) = self.doc_stack.pop() {
            self.resume_worker_sessions(
                frame.mermaid_session,
                frame.image_session,
                frame.document_prefetch_session,
            );
        }
    }

    fn resume_worker_sessions(
        &mut self,
        mermaid_session: MermaidSessionSnapshot,
        image_session: ImageSessionSnapshot,
        document_prefetch_session: DocumentPrefetchSessionSnapshot,
    ) {
        let terminal_size = self.view_state.terminal_size();
        self.mermaid_render.resume(
            mermaid_session,
            &self.document,
            &self.rendered,
            &self.picker,
            terminal_size,
        );
        self.image_render.resume(
            image_session,
            &self.document,
            &self.rendered,
            self.base_path.as_ref(),
            &self.picker,
            terminal_size,
        );
        self.document_prefetch.resume(document_prefetch_session);
    }

    fn capture_document_frame(&mut self) -> DocumentFrame {
        DocumentFrame {
            document: self.document.clone(),
            rendered: self.rendered.clone(),
            mermaid_session: self.mermaid_render.suspend(),
            image_session: self.image_render.suspend(),
            document_prefetch_session: self.document_prefetch.suspend(),
            document_cache: self.document_cache.clone(),
            preview_render_cache: self.preview.cache.clone(),
            pending_preview: self.preview.pending,
            preview_zoom: self.preview.zoom,
            toc_selected: self.preview.toc_selected,
            view_state: self.view_state.clone(),
            scroll_visual: self.scroll.visual,
            scroll_anim_speed: self.scroll.anim_speed,
            checklist_state: self.checklist_state.clone(),
            source_label: self.source_label.clone(),
            base_path: self.base_path.clone(),
            file_watch: self.file_watch.clone(),
            nav_stack: self.nav_stack.clone(),
            marks: self.marks.clone(),
            outline: self.outline,
            text_selection: self.text_selection,
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
        self.preview.cache.clear();
        self.preview.pending = None;
        self.view_state = crate::domain::ViewState::new(terminal_size);
        self.nav_stack.clear();
        self.checklist_state = ChecklistState::new(ChecklistStyle::from_env());
        self.base_path = Some(path.clone());
        self.source_label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        self.file_watch = FileWatch::new(path).ok();
        self.scroll.reset_to_top();
        self.help_visible = false;
        self.clear_marks();
        self.pending_input = super::pending::PendingInput::None;
        self.reset_transient_view_ui();
        self.mermaid_render.begin_document();
        self.image_render.begin_document();
        self.document_prefetch.begin_document();
        self.invalidate_prefetch_viewport();
        self.maybe_prefetch_visible_links();
        Ok(())
    }

    /// Reset UI that must not leak into a newly opened document.
    ///
    /// Outline visibility is kept so a sidebar stays open across file jumps;
    /// selection index is cleared and will resync from scroll on the next draw.
    fn reset_transient_view_ui(&mut self) {
        self.clear_text_selection();
        self.outline.selected = 0;
        self.outline.focused = false;
        self.reset_preview_zoom();
        self.preview.toc_selected = 0;
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
        self.preview.cache = frame.preview_render_cache;
        self.preview.pending = frame.pending_preview;
        self.preview.zoom = frame.preview_zoom;
        self.preview.toc_selected = frame.toc_selected;
        self.scroll.visual = frame.scroll_visual;
        self.scroll.anim_speed = frame.scroll_anim_speed;
        self.checklist_state = frame.checklist_state;
        self.source_label = frame.source_label;
        self.base_path = frame.base_path;
        self.file_watch = frame.file_watch;
        self.nav_stack = frame.nav_stack;
        self.marks = frame.marks;
        self.outline = frame.outline;
        self.text_selection = frame.text_selection;
        self.selection_drag = None;
        self.pending_input = super::pending::PendingInput::None;
        self.scroll.key_down_at = None;
        self.help_visible = false;
        self.invalidate_prefetch_viewport();
        self.resume_worker_sessions(
            frame.mermaid_session,
            frame.image_session,
            frame.document_prefetch_session,
        );
        Ok(())
    }
}
