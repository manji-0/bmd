//! Stack of previously viewed documents for in-app file navigation.

use std::path::PathBuf;

use crate::domain::{ChecklistState, Document, NavStack, ViewState};

use super::reload::FileWatch;

/// Snapshot of document viewing state pushed before opening another file.
#[derive(Clone)]
pub(crate) struct DocumentFrame {
    pub document: Document,
    pub view_state: ViewState,
    pub scroll_visual: f32,
    pub scroll_anim_speed: f32,
    pub tracked_scroll_position: f32,
    pub show_terminal_images: bool,
    pub checklist_state: ChecklistState,
    pub source_label: Option<String>,
    pub base_path: Option<PathBuf>,
    pub file_watch: Option<FileWatch>,
    pub nav_stack: NavStack,
}

/// Bottom-first stack of prior documents; the bottom entry is the root file.
#[derive(Default)]
pub(crate) struct DocStack {
    frames: Vec<DocumentFrame>,
}

impl DocStack {
    pub fn push(&mut self, frame: DocumentFrame) {
        self.frames.push(frame);
    }

    pub fn pop(&mut self) -> Option<DocumentFrame> {
        self.frames.pop()
    }

    pub fn root(&self) -> Option<&DocumentFrame> {
        self.frames.first()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
    }

    pub fn len_frames(&self) -> usize {
        self.frames.len()
    }
}
