//! Background mermaid/image workers over the domain [`PreviewLoadSession`].

use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::domain::{
    Document, ImageSource, LinkId, LinkKind, MermaidSource, PreviewCompletion, PreviewLoadError,
    PreviewLoadSession, PreviewLoadStatus, PreviewSessionSnapshot, PreviewSource,
    PreviewSpawnRequest, TerminalSize,
};
use crate::error::AppError;
use crate::render::{RenderedDocument, render_markdown_image_from_src, render_mermaid_from_source};

use super::worker_pool::WorkerPool;

/// Inputs shared by every preview worker spawned for the current document.
pub(crate) struct PreviewEnv<'a> {
    pub document: &'a Document,
    pub picker: &'a Picker,
    pub terminal: TerminalSize,
    pub base_path: Option<&'a Path>,
}

/// How one preview kind is cached and rendered.
trait PreviewJob: PreviewSource + Send + 'static {
    fn is_cached(rendered: &RenderedDocument, document: &Document, link_id: LinkId) -> bool;
    fn render(
        &self,
        picker: &Picker,
        terminal: TerminalSize,
        base_path: Option<&Path>,
    ) -> Result<Protocol, AppError>;
    fn store(self, link_id: LinkId, protocol: Protocol, rendered: &mut RenderedDocument);
}

impl PreviewJob for MermaidSource {
    fn is_cached(rendered: &RenderedDocument, _: &Document, link_id: LinkId) -> bool {
        rendered.mermaid_images.contains_key(&link_id.0)
    }

    fn render(
        &self,
        picker: &Picker,
        terminal: TerminalSize,
        _: Option<&Path>,
    ) -> Result<Protocol, AppError> {
        render_mermaid_from_source(self.as_str(), picker, terminal)
    }

    fn store(self, link_id: LinkId, protocol: Protocol, rendered: &mut RenderedDocument) {
        rendered.mermaid_images.insert(link_id.0, protocol);
    }
}

impl PreviewJob for ImageSource {
    fn is_cached(rendered: &RenderedDocument, document: &Document, link_id: LinkId) -> bool {
        document
            .links
            .get(link_id.0)
            .is_some_and(|link| rendered.markdown_images.contains_key(link.url.as_str()))
    }

    fn render(
        &self,
        picker: &Picker,
        terminal: TerminalSize,
        base_path: Option<&Path>,
    ) -> Result<Protocol, AppError> {
        render_markdown_image_from_src(self.as_str(), base_path, picker, terminal)
    }

    fn store(self, _: LinkId, protocol: Protocol, rendered: &mut RenderedDocument) {
        rendered
            .markdown_images
            .insert(self.as_str().to_string(), protocol);
    }
}

struct WorkerResult<S> {
    completion: PreviewCompletion,
    source: S,
    protocol: Option<Protocol>,
}

/// Runs background renders of one preview kind and applies domain state transitions.
struct PreviewRenderPool<S> {
    session: PreviewLoadSession<S>,
    receiver: mpsc::Receiver<WorkerResult<S>>,
    sender: mpsc::Sender<WorkerResult<S>>,
    worker_pool: Arc<WorkerPool>,
}

impl<S: PreviewJob> PreviewRenderPool<S> {
    fn new(worker_pool: Arc<WorkerPool>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            session: PreviewLoadSession::default(),
            receiver,
            sender,
            worker_pool,
        }
    }

    fn transition(
        &mut self,
        env: &PreviewEnv,
        step: impl FnOnce(PreviewLoadSession<S>) -> (PreviewLoadSession<S>, Vec<PreviewSpawnRequest<S>>),
    ) {
        let (session, spawns) = step(mem::take(&mut self.session));
        self.session = session;
        for request in spawns {
            self.spawn(request, env);
        }
    }

    fn resume(
        &mut self,
        snapshot: PreviewSessionSnapshot<S>,
        rendered: &RenderedDocument,
        env: &PreviewEnv,
    ) {
        let is_cached = |id| S::is_cached(rendered, env.document, id);
        self.transition(env, |_| {
            PreviewLoadSession::resume(snapshot, env.document, is_cached)
        });
    }

    fn prefetch_visible(
        &mut self,
        visible: &[LinkId],
        rendered: &RenderedDocument,
        env: &PreviewEnv,
    ) {
        let is_cached = |id| S::is_cached(rendered, env.document, id);
        self.transition(env, |session| {
            session.schedule_visible_prefetch(visible, env.document, is_cached)
        });
    }

    fn request(&mut self, link_id: LinkId, rendered: &RenderedDocument, env: &PreviewEnv) {
        let cached = S::is_cached(rendered, env.document, link_id);
        self.transition(env, |session| {
            session.request(link_id, env.document, cached)
        });
    }

    fn poll(&mut self, rendered: &mut RenderedDocument, env: &PreviewEnv) -> bool {
        let mut dirty = false;
        while let Ok(result) = self.receiver.try_recv() {
            let link_id = result.completion.link_id;
            let mut applied = None;
            self.transition(env, |session| {
                let (session, outcome, spawns) =
                    session.apply_completion(result.completion, env.document);
                applied = Some(outcome);
                (session, spawns)
            });
            if applied.is_some_and(|applied| !applied.is_stale()) {
                dirty = true;
                if let Some(protocol) = result.protocol {
                    result.source.store(link_id, protocol, rendered);
                }
            }
        }
        dirty
    }

    fn status(
        &self,
        link_id: LinkId,
        rendered: &RenderedDocument,
        document: &Document,
    ) -> PreviewLoadStatus {
        self.session
            .preview_status(link_id, S::is_cached(rendered, document, link_id))
    }

    fn spawn(&self, request: PreviewSpawnRequest<S>, env: &PreviewEnv) {
        let sender = self.sender.clone();
        let picker = env.picker.clone();
        let terminal = env.terminal;
        let base_path = env.base_path.map(PathBuf::from);
        self.worker_pool.spawn(move || {
            let PreviewSpawnRequest {
                link_id,
                source,
                generation,
            } = request;
            let rendered = source.render(&picker, terminal, base_path.as_deref());
            let (outcome, protocol) = match rendered {
                Ok(protocol) => (Ok(()), Some(protocol)),
                Err(error) => (Err(PreviewLoadError::Load(error.to_string())), None),
            };
            let _ = sender.send(WorkerResult {
                completion: PreviewCompletion {
                    link_id,
                    generation,
                    outcome,
                },
                source,
                protocol,
            });
        });
    }
}

/// Preview session state parked with a document on the navigation stack.
#[derive(Clone, Default)]
pub(crate) struct PreviewSnapshots {
    mermaid: PreviewSessionSnapshot<MermaidSource>,
    image: PreviewSessionSnapshot<ImageSource>,
}

/// Mermaid and markdown image render pools for the active document.
pub(crate) struct PreviewPools {
    mermaid: PreviewRenderPool<MermaidSource>,
    image: PreviewRenderPool<ImageSource>,
}

impl PreviewPools {
    pub(crate) fn new(worker_pool: &Arc<WorkerPool>) -> Self {
        Self {
            mermaid: PreviewRenderPool::new(Arc::clone(worker_pool)),
            image: PreviewRenderPool::new(Arc::clone(worker_pool)),
        }
    }

    pub(crate) fn begin_document(&mut self) {
        self.mermaid.session = mem::take(&mut self.mermaid.session).begin_document();
        self.image.session = mem::take(&mut self.image.session).begin_document();
    }

    pub(crate) fn suspend(&self) -> PreviewSnapshots {
        PreviewSnapshots {
            mermaid: self.mermaid.session.clone().suspend(),
            image: self.image.session.clone().suspend(),
        }
    }

    pub(crate) fn resume(
        &mut self,
        snapshots: PreviewSnapshots,
        rendered: &RenderedDocument,
        env: &PreviewEnv,
    ) {
        self.mermaid.resume(snapshots.mermaid, rendered, env);
        self.image.resume(snapshots.image, rendered, env);
    }

    pub(crate) fn prefetch_visible(
        &mut self,
        visible: &[LinkId],
        rendered: &RenderedDocument,
        env: &PreviewEnv,
    ) {
        self.mermaid.prefetch_visible(visible, rendered, env);
        self.image.prefetch_visible(visible, rendered, env);
    }

    /// Request one preview link; other link kinds are ignored.
    pub(crate) fn request(
        &mut self,
        link_id: LinkId,
        rendered: &RenderedDocument,
        env: &PreviewEnv,
    ) {
        match env.document.links.get(link_id.0).map(|link| link.kind) {
            Some(LinkKind::Mermaid) => self.mermaid.request(link_id, rendered, env),
            Some(LinkKind::Image) => self.image.request(link_id, rendered, env),
            _ => {}
        }
    }

    /// Apply finished renders; returns true when any preview changed.
    pub(crate) fn poll(&mut self, rendered: &mut RenderedDocument, env: &PreviewEnv) -> bool {
        let mermaid = self.mermaid.poll(rendered, env);
        let image = self.image.poll(rendered, env);
        mermaid || image
    }

    pub(crate) fn status(
        &self,
        link_id: LinkId,
        rendered: &RenderedDocument,
        document: &Document,
    ) -> PreviewLoadStatus {
        match document.links.get(link_id.0).map(|link| link.kind) {
            Some(LinkKind::Mermaid) => self.mermaid.status(link_id, rendered, document),
            Some(LinkKind::Image) => self.image.status(link_id, rendered, document),
            Some(LinkKind::Toc) => PreviewLoadStatus::Ready,
            _ => PreviewLoadStatus::Idle,
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.mermaid.session.has_in_flight() || self.image.session.has_in_flight()
    }
}
