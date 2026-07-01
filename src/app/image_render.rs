//! Background markdown image worker adapter over [`ImageRenderSession`].

use std::mem;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::domain::{
    Document, ImageCompletion, ImageRenderError, ImageRenderSession, ImageSessionSnapshot,
    ImageSpawnRequest, LinkId, PreviewLoadCompletionApplied, PreviewLoadStatus, TerminalSize,
};
use crate::render::{RenderedDocument, render_markdown_image_from_src};

use super::worker_pool::WorkerPool;

struct WorkerResult {
    completion: ImageCompletion,
    protocol: Option<Protocol>,
    src: String,
}

/// Runs background markdown image loads and applies domain state transitions.
pub(crate) struct ImageRenderPool {
    session: ImageRenderSession,
    receiver: mpsc::Receiver<WorkerResult>,
    sender: mpsc::Sender<WorkerResult>,
    worker_pool: Arc<WorkerPool>,
}

impl ImageRenderPool {
    pub(crate) fn new(worker_pool: Arc<WorkerPool>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            session: ImageRenderSession::new(),
            receiver,
            sender,
            worker_pool,
        }
    }

    pub fn begin_document(&mut self) {
        let session = mem::take(&mut self.session);
        self.session = session.begin_document();
    }

    pub fn suspend(&self) -> ImageSessionSnapshot {
        self.session.clone().suspend()
    }

    pub fn resume(
        &mut self,
        snapshot: ImageSessionSnapshot,
        document: &Document,
        rendered: &RenderedDocument,
        base_path: Option<&PathBuf>,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let is_cached = |link_id: LinkId| {
            rendered
                .markdown_images
                .contains_key(image_url(document, link_id).as_str())
        };
        let (session, spawns) = ImageRenderSession::resume(
            snapshot,
            document,
            base_path.map(PathBuf::as_path),
            is_cached,
        );
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn prefetch_visible(
        &mut self,
        visible: &[LinkId],
        document: &Document,
        rendered: &RenderedDocument,
        base_path: Option<&PathBuf>,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let is_cached = |link_id: LinkId| {
            rendered
                .markdown_images
                .contains_key(image_url(document, link_id).as_str())
        };
        let session = mem::take(&mut self.session);
        let (session, spawns) = session.schedule_visible_prefetch(
            visible,
            document,
            base_path.map(PathBuf::as_path),
            is_cached,
        );
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn request(
        &mut self,
        link_id: LinkId,
        document: &Document,
        rendered: &RenderedDocument,
        base_path: Option<&PathBuf>,
        picker: &Picker,
        terminal: TerminalSize,
    ) {
        let src = image_url(document, link_id);
        let cached = rendered.markdown_images.contains_key(src.as_str());
        let Ok((session, spawns)) = mem::take(&mut self.session).request(
            link_id,
            document,
            base_path.map(PathBuf::as_path),
            cached,
        ) else {
            return;
        };
        self.session = session;
        self.spawn_all(spawns, picker, terminal);
    }

    pub fn poll(
        &mut self,
        rendered: &mut RenderedDocument,
        document: &Document,
        base_path: Option<&PathBuf>,
        picker: &Picker,
        terminal: TerminalSize,
    ) -> bool {
        let mut dirty = false;
        while let Ok(result) = self.receiver.try_recv() {
            let session = mem::take(&mut self.session);
            let (session, applied, spawns) = session.apply_completion(
                result.completion.clone(),
                document,
                base_path.map(PathBuf::as_path),
            );
            self.session = session;
            if !matches!(applied, PreviewLoadCompletionApplied::Stale)
                && let Some(protocol) = result.protocol
            {
                rendered.markdown_images.insert(result.src, protocol);
            }
            if !matches!(applied, PreviewLoadCompletionApplied::Stale) {
                dirty = true;
            }
            self.spawn_all(spawns, picker, terminal);
        }
        dirty
    }

    pub fn preview_status(
        &self,
        link_id: LinkId,
        document: &Document,
        rendered: &RenderedDocument,
    ) -> PreviewLoadStatus {
        let src = image_url(document, link_id);
        let cached = rendered.markdown_images.contains_key(src.as_str());
        self.session.preview_status(link_id, cached)
    }

    pub fn has_pending(&self) -> bool {
        self.session.has_in_flight()
    }

    fn spawn_all(&self, spawns: Vec<ImageSpawnRequest>, picker: &Picker, terminal: TerminalSize) {
        for request in spawns {
            self.spawn_one(request, picker.clone(), terminal);
        }
    }

    fn spawn_one(&self, request: ImageSpawnRequest, picker: Picker, terminal: TerminalSize) {
        let src = request.src.as_str().to_string();
        let base_path = request.base_path;
        let sender = self.sender.clone();
        let link_id = request.link_id;
        let generation = request.generation;
        let worker_pool = Arc::clone(&self.worker_pool);
        worker_pool.spawn(move || {
            let (completion, protocol) =
                match render_markdown_image_from_src(&src, base_path.as_deref(), &picker, terminal)
                {
                    Ok(protocol) => (
                        ImageCompletion {
                            link_id,
                            generation,
                            outcome: Ok(()),
                        },
                        Some(protocol),
                    ),
                    Err(error) => (
                        ImageCompletion {
                            link_id,
                            generation,
                            outcome: Err(ImageRenderError::Load(error.to_string())),
                        },
                        None,
                    ),
                };
            let _ = sender.send(WorkerResult {
                completion,
                protocol,
                src,
            });
        });
    }
}

fn image_url(document: &Document, link_id: LinkId) -> String {
    document.links[link_id.0].url.as_str().to_string()
}
