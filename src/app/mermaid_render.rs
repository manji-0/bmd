//! Background mermaid worker adapter over [`MermaidRenderSession`].

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::domain::{
    Document, LinkId, MermaidCompletion, MermaidCompletionApplied, MermaidRenderError,
    MermaidRenderSession, MermaidSessionSnapshot, MermaidSpawnRequest, PreviewLoadStatus,
    TerminalSize,
};
use crate::render::{RenderedDocument, render_mermaid_from_source};

struct WorkerResult {
    completion: MermaidCompletion,
    protocol: Option<Protocol>,
}

/// Runs background mermaid renders and applies domain state transitions.
pub(crate) struct MermaidRenderPool {
    session: MermaidRenderSession,
    receiver: Receiver<WorkerResult>,
    sender: Sender<WorkerResult>,
}

impl Default for MermaidRenderPool {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            session: MermaidRenderSession::new(),
            receiver,
            sender,
        }
    }
}

impl MermaidRenderPool {
    pub fn begin_document(&mut self) {
        self.session = self.session.clone().begin_document();
    }

    pub fn suspend(&self) -> MermaidSessionSnapshot {
        self.session.clone().suspend()
    }

    pub fn resume(
        &mut self,
        snapshot: MermaidSessionSnapshot,
        document: &Document,
        rendered: &RenderedDocument,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let is_cached = |link_id: LinkId| rendered.mermaid_images.contains_key(&link_id.0);
        let (session, spawns) = MermaidRenderSession::resume(snapshot, document, is_cached);
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn prefetch(
        &mut self,
        document: &Document,
        rendered: &RenderedDocument,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let is_cached = |link_id: LinkId| rendered.mermaid_images.contains_key(&link_id.0);
        let (session, spawns) = self.session.clone().schedule_prefetch(document, is_cached);
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn request(
        &mut self,
        link_id: LinkId,
        document: &Document,
        rendered: &RenderedDocument,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let cached = rendered.mermaid_images.contains_key(&link_id.0);
        let Ok((session, spawns)) = self.session.clone().request(link_id, document, cached) else {
            return;
        };
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn poll(
        &mut self,
        rendered: &mut RenderedDocument,
        document: &Document,
        picker: &Picker,
        terminal: TerminalSize,
    ) -> bool {
        let mut dirty = false;
        while let Ok(result) = self.receiver.try_recv() {
            let (session, applied, spawns) = self
                .session
                .clone()
                .apply_completion(result.completion.clone(), document);
            self.session = session;
            if !matches!(applied, MermaidCompletionApplied::Stale)
                && let Some(protocol) = result.protocol
            {
                rendered
                    .mermaid_images
                    .insert(result.completion.link_id.0, protocol);
            }
            if !matches!(applied, MermaidCompletionApplied::Stale) {
                dirty = true;
            }
            self.spawn_all(spawns, picker, terminal);
        }
        dirty
    }

    pub fn preview_status(
        &self,
        link_id: LinkId,
        rendered: &RenderedDocument,
    ) -> PreviewLoadStatus {
        let cached = rendered.mermaid_images.contains_key(&link_id.0);
        match self.session.preview_status(link_id, cached) {
            crate::domain::MermaidPreviewStatus::Idle => PreviewLoadStatus::Idle,
            crate::domain::MermaidPreviewStatus::Queued => PreviewLoadStatus::Queued,
            crate::domain::MermaidPreviewStatus::Rendering => PreviewLoadStatus::Loading,
            crate::domain::MermaidPreviewStatus::Ready => PreviewLoadStatus::Ready,
            crate::domain::MermaidPreviewStatus::Failed => PreviewLoadStatus::Failed,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.session.has_in_flight()
    }

    fn spawn_all(&self, spawns: Vec<MermaidSpawnRequest>, picker: &Picker, terminal: TerminalSize) {
        for request in spawns {
            self.spawn_one(request, picker.clone(), terminal);
        }
    }

    fn spawn_one(&self, request: MermaidSpawnRequest, picker: Picker, terminal: TerminalSize) {
        let source = request.source.as_str().to_string();
        let sender = self.sender.clone();
        let link_id = request.link_id;
        let generation = request.generation;
        thread::spawn(move || {
            let (completion, protocol) =
                match render_mermaid_from_source(&source, &picker, terminal) {
                    Ok(protocol) => (
                        MermaidCompletion {
                            link_id,
                            generation,
                            outcome: Ok(()),
                        },
                        Some(protocol),
                    ),
                    Err(error) => (
                        MermaidCompletion {
                            link_id,
                            generation,
                            outcome: Err(MermaidRenderError::Render(error.to_string())),
                        },
                        None,
                    ),
                };
            let _ = sender.send(WorkerResult {
                completion,
                protocol,
            });
        });
    }
}
