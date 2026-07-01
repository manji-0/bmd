//! In-app navigation to linked markdown files.

use std::path::PathBuf;

use crate::domain::{
    ChecklistState, ChecklistStyle, document_link_path_part, resolve_document_path,
};
use crate::error::AppError;
use crate::parse::parse;
use crate::render::{DocumentRenderCache, RenderedDocument};

use super::App;
use super::doc_stack::DocumentFrame;
use super::reload::FileWatch;
use super::scroll::SCROLL_ANIM_SPEED;

impl App {
    pub(crate) fn open_document_link(&mut self, dest: &str) {
        let resolved = match resolve_document_path(self.base_path.as_deref(), dest) {
            Ok(path) => path,
            Err(e) => {
                self.set_status_message(e.to_string());
                return;
            }
        };
        if !resolved.is_file() {
            self.set_status_message(format!("file not found: {}", resolved.display()));
            return;
        }

        let content = match std::fs::read_to_string(&resolved) {
            Ok(content) => content,
            Err(e) => {
                self.set_status_message(format!("read failed: {e}"));
                return;
            }
        };
        let document = match parse(&content) {
            Ok(document) => document,
            Err(e) => {
                self.set_status_message(format!("parse error: {e}"));
                return;
            }
        };

        let anchor = document_link_path_part(dest).1;
        let frame = self.capture_document_frame();
        self.doc_stack.push(frame);

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

    pub(crate) fn doc_back(&mut self) {
        let Some(frame) = self.doc_stack.pop() else {
            self.set_status_message("document stack empty".into());
            return;
        };
        match self.try_restore_document_frame(frame) {
            Ok(()) => {}
            Err(err) => {
                let (e, frame) = *err;
                self.set_status_message(e.to_string());
                self.doc_stack.push(frame);
            }
        }
    }

    pub(crate) fn doc_reset(&mut self) {
        let Some(root) = self.doc_stack.root().cloned() else {
            return;
        };
        match self.try_restore_document_frame(root) {
            Ok(()) => self.doc_stack.clear(),
            Err(err) => self.set_status_message(err.0.to_string()),
        }
    }

    fn capture_document_frame(&self) -> DocumentFrame {
        DocumentFrame {
            document: self.document.clone(),
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
        self.document_cache = DocumentRenderCache::default();
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
        let terminal_size = frame.view_state.terminal_size();
        let rendered = match RenderedDocument::new(
            &frame.document,
            &self.picker,
            terminal_size,
            frame.base_path.as_deref(),
        ) {
            Ok(rendered) => rendered,
            Err(e) => return Err(Box::new((e, frame))),
        };
        self.document = frame.document;
        self.rendered = rendered;
        self.view_state = frame.view_state;
        self.document_cache = DocumentRenderCache::default();
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
        Ok(())
    }
}
