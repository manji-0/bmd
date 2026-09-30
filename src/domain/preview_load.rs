//! Per-link preview load lifecycle shared by mermaid diagrams and markdown images.
//!
//! Each preview link progresses independently: `(absent) → Queued → Loading → Ready | Failed`.
//! [`DocumentGeneration`] invalidates in-flight work when the active document changes.

use std::collections::{HashMap, HashSet, VecDeque};
use std::marker::PhantomData;

use super::document_generation::DocumentGeneration;
use super::link::{LinkId, LinkKind};
use super::markdown::Document;

/// Maximum concurrent background loads per preview kind.
const MAX_IN_FLIGHT: usize = 2;

/// Source payload a background worker needs to produce one preview image.
pub trait PreviewSource: Sized {
    const LINK_KIND: LinkKind;

    /// Resolve the source for `link_id` in `document`.
    ///
    /// # Errors
    ///
    /// Returns [`PreviewLoadError`] when the link has another kind or its source is missing.
    fn for_link(document: &Document, link_id: LinkId) -> Result<Self, PreviewLoadError>;
}

/// Non-empty mermaid diagram source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidSource(String);

impl MermaidSource {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreviewSource for MermaidSource {
    const LINK_KIND: LinkKind = LinkKind::Mermaid;

    fn for_link(document: &Document, link_id: LinkId) -> Result<Self, PreviewLoadError> {
        let url = preview_link_url(document, link_id, Self::LINK_KIND)?;
        mermaid_diagram_index(url)
            .and_then(|idx| document.mermaid_diagrams.get(idx))
            .map(|diagram| Self(diagram.source.clone()))
            .ok_or(PreviewLoadError::SourceMissing(link_id))
    }
}

/// Image URL/path referenced by a markdown image link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSource(String);

impl ImageSource {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreviewSource for ImageSource {
    const LINK_KIND: LinkKind = LinkKind::Image;

    fn for_link(document: &Document, link_id: LinkId) -> Result<Self, PreviewLoadError> {
        preview_link_url(document, link_id, Self::LINK_KIND).map(|url| Self(url.to_string()))
    }
}

fn preview_link_url(
    document: &Document,
    link_id: LinkId,
    kind: LinkKind,
) -> Result<&str, PreviewLoadError> {
    let link = document
        .links
        .get(link_id.0)
        .ok_or(PreviewLoadError::SourceMissing(link_id))?;
    if link.kind != kind {
        return Err(PreviewLoadError::WrongKind(link_id));
    }
    Ok(link.url.as_str())
}

pub fn mermaid_diagram_index(url: &str) -> Option<usize> {
    url.strip_prefix("bmd:mermaid:")?.parse().ok()
}

/// Domain errors for preview scheduling and completion.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreviewLoadError {
    #[error("link {0} has a different preview kind")]
    WrongKind(LinkId),
    #[error("preview source missing for {0}")]
    SourceMissing(LinkId),
    #[error("load failed: {0}")]
    Load(String),
}

/// Lifecycle state of one preview link's terminal image. Absent means idle.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PreviewLoadTask {
    Queued,
    Loading { generation: DocumentGeneration },
    Ready,
    Failed,
}

/// UI-facing preview status derived from task state and render cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewLoadStatus {
    Idle,
    Queued,
    Loading,
    Ready,
    Failed,
}

/// Outcome applied to session state after accepting a completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewLoadCompletionApplied {
    Ready { link_id: LinkId },
    Failed { link_id: LinkId },
    Stale,
}

impl PreviewLoadCompletionApplied {
    pub fn is_stale(self) -> bool {
        matches!(self, Self::Stale)
    }
}

/// Work item handed to the infrastructure layer for background loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSpawnRequest<S> {
    pub link_id: LinkId,
    pub source: S,
    pub generation: DocumentGeneration,
}

/// Completion event from a background worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewCompletion {
    pub link_id: LinkId,
    pub generation: DocumentGeneration,
    pub outcome: Result<(), PreviewLoadError>,
}

/// Session state parked on the document stack; in-flight work is re-queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSessionSnapshot<S>(PreviewLoadSession<S>);

impl<S> Default for PreviewSessionSnapshot<S> {
    fn default() -> Self {
        Self(PreviewLoadSession::default())
    }
}

type Spawns<S> = Vec<PreviewSpawnRequest<S>>;

/// Aggregate preview load state for the current document.
#[derive(Debug, PartialEq, Eq)]
pub struct PreviewLoadSession<S> {
    generation: DocumentGeneration,
    tasks: HashMap<LinkId, PreviewLoadTask>,
    queue: VecDeque<LinkId>,
    queued: HashSet<LinkId>,
    source: PhantomData<fn() -> S>,
}

impl<S> Clone for PreviewLoadSession<S> {
    fn clone(&self) -> Self {
        Self {
            generation: self.generation,
            tasks: self.tasks.clone(),
            queue: self.queue.clone(),
            queued: self.queued.clone(),
            source: PhantomData,
        }
    }
}

impl<S> Default for PreviewLoadSession<S> {
    fn default() -> Self {
        Self {
            generation: DocumentGeneration::INITIAL,
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
            source: PhantomData,
        }
    }
}

impl<S: PreviewSource> PreviewLoadSession<S> {
    pub fn has_in_flight(&self) -> bool {
        self.count_in_flight() > 0 || !self.queue.is_empty()
    }

    pub fn preview_status(&self, link_id: LinkId, cached: bool) -> PreviewLoadStatus {
        if cached {
            return PreviewLoadStatus::Ready;
        }
        match self.tasks.get(&link_id) {
            None => PreviewLoadStatus::Idle,
            Some(PreviewLoadTask::Queued) => PreviewLoadStatus::Queued,
            Some(PreviewLoadTask::Loading { .. }) => PreviewLoadStatus::Loading,
            Some(PreviewLoadTask::Ready) => PreviewLoadStatus::Ready,
            Some(PreviewLoadTask::Failed) => PreviewLoadStatus::Failed,
        }
    }

    /// Invalidate in-flight work and clear scheduling state for a new document.
    pub fn begin_document(self) -> Self {
        Self {
            generation: self.generation.next(),
            ..Self::default()
        }
    }

    /// Queue visible links of this kind that are not already cached or in flight.
    pub fn schedule_visible_prefetch(
        mut self,
        visible: &[LinkId],
        document: &Document,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Spawns<S>) {
        for &link_id in visible {
            if document
                .links
                .get(link_id.0)
                .is_none_or(|link| link.kind != S::LINK_KIND)
            {
                continue;
            }
            if is_cached(link_id) {
                self.mark_ready(link_id);
            } else {
                let _ = self.try_enqueue(link_id, document);
            }
        }
        self.drain_spawns(document)
    }

    /// Queue one link, then start workers up to the concurrency limit.
    /// Links of another kind or without a source are ignored.
    pub fn request(
        mut self,
        link_id: LinkId,
        document: &Document,
        is_cached: bool,
    ) -> (Self, Spawns<S>) {
        if is_cached {
            self.mark_ready(link_id);
            return (self, Vec::new());
        }
        if self.try_enqueue(link_id, document).is_err() {
            return (self, Vec::new());
        }
        self.drain_spawns(document)
    }

    /// Apply a worker completion and drain additional spawns if slots opened.
    pub fn apply_completion(
        mut self,
        completion: PreviewCompletion,
        document: &Document,
    ) -> (Self, PreviewLoadCompletionApplied, Spawns<S>) {
        let applied = self.record_completion(completion);
        if applied.is_stale() {
            return (self, applied, Vec::new());
        }
        let (session, spawns) = self.drain_spawns(document);
        (session, applied, spawns)
    }

    /// Move this session into a snapshot and leave an empty session at the next
    /// generation so in-flight completions for the captured document are stale.
    pub fn detach_snapshot(self) -> (Self, PreviewSessionSnapshot<S>) {
        let live = Self {
            generation: self.generation.next(),
            ..Self::default()
        };
        (live, self.suspend())
    }

    /// Re-queue in-flight tasks before pushing this document onto the navigation stack.
    pub fn suspend(mut self) -> PreviewSessionSnapshot<S> {
        for (link_id, task) in &mut self.tasks {
            if matches!(task, PreviewLoadTask::Loading { .. }) {
                *task = PreviewLoadTask::Queued;
                if self.queued.insert(*link_id) {
                    self.queue.push_back(*link_id);
                }
            }
        }
        PreviewSessionSnapshot(self)
    }

    /// Restore session state after document navigation and resume background work.
    pub fn resume(
        PreviewSessionSnapshot(mut session): PreviewSessionSnapshot<S>,
        document: &Document,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Spawns<S>) {
        let cached: Vec<LinkId> = session
            .tasks
            .keys()
            .copied()
            .filter(|&id| is_cached(id))
            .collect();
        for link_id in cached {
            session.mark_ready(link_id);
        }
        session.drain_spawns(document)
    }

    fn mark_ready(&mut self, link_id: LinkId) {
        self.tasks.insert(link_id, PreviewLoadTask::Ready);
        if self.queued.remove(&link_id) {
            self.queue.retain(|id| *id != link_id);
        }
    }

    fn try_enqueue(
        &mut self,
        link_id: LinkId,
        document: &Document,
    ) -> Result<(), PreviewLoadError> {
        S::for_link(document, link_id)?;
        if matches!(
            self.tasks.get(&link_id),
            Some(
                PreviewLoadTask::Queued | PreviewLoadTask::Loading { .. } | PreviewLoadTask::Ready
            )
        ) {
            return Ok(());
        }
        self.tasks.insert(link_id, PreviewLoadTask::Queued);
        if self.queued.insert(link_id) {
            self.queue.push_back(link_id);
        }
        Ok(())
    }

    fn drain_spawns(mut self, document: &Document) -> (Self, Spawns<S>) {
        let mut spawns = Vec::new();
        let mut slots = MAX_IN_FLIGHT.saturating_sub(self.count_in_flight());
        while slots > 0 {
            let Some(link_id) = self.queue.pop_front() else {
                break;
            };
            self.queued.remove(&link_id);
            if !matches!(self.tasks.get(&link_id), Some(PreviewLoadTask::Queued)) {
                continue;
            }
            let Ok(source) = S::for_link(document, link_id) else {
                self.tasks.insert(link_id, PreviewLoadTask::Failed);
                continue;
            };
            let generation = self.generation;
            self.tasks
                .insert(link_id, PreviewLoadTask::Loading { generation });
            spawns.push(PreviewSpawnRequest {
                link_id,
                source,
                generation,
            });
            slots -= 1;
        }
        (self, spawns)
    }

    fn record_completion(&mut self, completion: PreviewCompletion) -> PreviewLoadCompletionApplied {
        let link_id = completion.link_id;
        match self.tasks.get(&link_id) {
            Some(PreviewLoadTask::Loading { generation })
                if *generation == completion.generation => {}
            _ => return PreviewLoadCompletionApplied::Stale,
        }
        if completion.outcome.is_ok() {
            self.tasks.insert(link_id, PreviewLoadTask::Ready);
            PreviewLoadCompletionApplied::Ready { link_id }
        } else {
            self.tasks.insert(link_id, PreviewLoadTask::Failed);
            PreviewLoadCompletionApplied::Failed { link_id }
        }
    }

    fn count_in_flight(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| matches!(task, PreviewLoadTask::Loading { .. }))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Link, LinkUrl, MermaidDiagram};

    type MermaidSession = PreviewLoadSession<MermaidSource>;

    fn document(mermaid: usize, images: usize) -> Document {
        let mermaid_links = (0..mermaid).map(|i| (format!("bmd:mermaid:{i}"), LinkKind::Mermaid));
        let image_links = (0..images).map(|i| (format!("assets/{i}.png"), LinkKind::Image));
        Document {
            blocks: vec![],
            links: mermaid_links
                .chain(image_links)
                .map(|(url, kind)| Link {
                    url: LinkUrl::new(url).unwrap(),
                    title: None,
                    kind,
                })
                .collect(),
            mermaid_diagrams: (0..mermaid)
                .map(|i| MermaidDiagram {
                    source: format!("graph TD; N{i};"),
                })
                .collect(),
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        }
    }

    fn status(session: &MermaidSession, id: usize) -> PreviewLoadStatus {
        session.preview_status(LinkId(id), false)
    }

    fn complete(
        session: MermaidSession,
        spawn: &PreviewSpawnRequest<MermaidSource>,
        outcome: Result<(), PreviewLoadError>,
        document: &Document,
    ) -> (MermaidSession, PreviewLoadCompletionApplied) {
        let completion = PreviewCompletion {
            link_id: spawn.link_id,
            generation: spawn.generation,
            outcome,
        };
        let (session, applied, _) = session.apply_completion(completion, document);
        (session, applied)
    }

    #[test]
    fn request_spawns_with_resolved_source() {
        let doc = document(1, 0);
        let (session, spawns) = MermaidSession::default().request(LinkId(0), &doc, false);
        assert_eq!(status(&session, 0), PreviewLoadStatus::Loading);
        assert_eq!(spawns[0].source.as_str(), "graph TD; N0;");
        assert_eq!(spawns[0].generation, DocumentGeneration::INITIAL);
    }

    #[test]
    fn request_ignores_wrong_kind_and_missing_links() {
        let doc = document(1, 1);
        for id in [LinkId(1), LinkId(9)] {
            let (session, spawns) = MermaidSession::default().request(id, &doc, false);
            assert!(spawns.is_empty());
            assert!(!session.has_in_flight());
        }
        assert_eq!(
            MermaidSource::for_link(&doc, LinkId(1)),
            Err(PreviewLoadError::WrongKind(LinkId(1)))
        );
        assert_eq!(
            MermaidSource::for_link(&doc, LinkId(9)),
            Err(PreviewLoadError::SourceMissing(LinkId(9)))
        );
        let (_, spawns) =
            PreviewLoadSession::<ImageSource>::default().request(LinkId(1), &doc, false);
        assert_eq!(spawns[0].source.as_str(), "assets/0.png");
    }

    #[test]
    fn concurrency_is_capped_and_completion_drains_queue() {
        let doc = document(3, 0);
        let all = [LinkId(0), LinkId(1), LinkId(2)];
        let (session, spawns) =
            MermaidSession::default().schedule_visible_prefetch(&all, &doc, |_| false);
        assert_eq!(spawns.len(), MAX_IN_FLIGHT);
        assert_eq!(status(&session, 2), PreviewLoadStatus::Queued);
        let (session, applied, next) = session.apply_completion(
            PreviewCompletion {
                link_id: LinkId(0),
                generation: spawns[0].generation,
                outcome: Ok(()),
            },
            &doc,
        );
        assert_eq!(
            applied,
            PreviewLoadCompletionApplied::Ready { link_id: LinkId(0) }
        );
        assert_eq!(next[0].link_id, LinkId(2));
        assert_eq!(status(&session, 0), PreviewLoadStatus::Ready);
    }

    #[test]
    fn failed_completion_is_retried_on_next_request() {
        let doc = document(1, 0);
        let (session, spawns) = MermaidSession::default().request(LinkId(0), &doc, false);
        let err = Err(PreviewLoadError::Load("boom".into()));
        let (session, applied) = complete(session, &spawns[0], err, &doc);
        assert_eq!(
            applied,
            PreviewLoadCompletionApplied::Failed { link_id: LinkId(0) }
        );
        assert_eq!(status(&session, 0), PreviewLoadStatus::Failed);
        let (session, spawns) = session.request(LinkId(0), &doc, false);
        assert_eq!(spawns.len(), 1);
        assert_eq!(status(&session, 0), PreviewLoadStatus::Loading);
    }

    #[test]
    fn completion_from_old_generation_is_stale() {
        let doc = document(1, 0);
        let (session, spawns) = MermaidSession::default().request(LinkId(0), &doc, false);
        let session = session.begin_document();
        let (session, spawns_again) = session.request(LinkId(0), &doc, false);
        assert_ne!(spawns[0].generation, spawns_again[0].generation);
        let (session, applied) = complete(session, &spawns[0], Ok(()), &doc);
        assert!(applied.is_stale());
        assert_eq!(status(&session, 0), PreviewLoadStatus::Loading);
    }

    #[test]
    fn prefetch_skips_other_kinds_and_marks_cached_ready() {
        let doc = document(2, 1);
        let visible = [LinkId(0), LinkId(1), LinkId(2)];
        let (session, spawns) =
            MermaidSession::default().schedule_visible_prefetch(&visible, &doc, |id| id.0 == 0);
        assert_eq!(status(&session, 0), PreviewLoadStatus::Ready);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].link_id, LinkId(1));
        assert_eq!(status(&session, 2), PreviewLoadStatus::Idle);
    }

    #[test]
    fn suspend_requeues_in_flight_and_resume_respawns() {
        let doc = document(2, 0);
        let (session, _) = MermaidSession::default().request(LinkId(0), &doc, false);
        let (session, _) = session.request(LinkId(1), &doc, false);
        let snapshot = session.suspend();
        assert_eq!(
            snapshot.0.preview_status(LinkId(0), false),
            PreviewLoadStatus::Queued
        );
        let (session, spawns) = MermaidSession::resume(snapshot, &doc, |id| id.0 == 1);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].link_id, LinkId(0));
        assert_eq!(status(&session, 1), PreviewLoadStatus::Ready);
        assert!(session.has_in_flight());
    }

    #[test]
    fn detach_snapshot_makes_in_flight_completions_stale_until_resume() {
        let doc = document(1, 0);
        let (session, spawns) = MermaidSession::default().request(LinkId(0), &doc, false);
        let generation = spawns[0].generation;
        let (live, snapshot) = session.detach_snapshot();
        assert!(!live.has_in_flight());
        let (_, applied) = complete(live, &spawns[0], Ok(()), &doc);
        assert!(applied.is_stale());
        let (restored, resumed) = MermaidSession::resume(snapshot, &doc, |_| false);
        assert_eq!(resumed[0].generation, generation);
        assert_eq!(status(&restored, 0), PreviewLoadStatus::Loading);
    }

    #[test]
    fn cached_status_wins() {
        let session = MermaidSession::default();
        assert_eq!(status(&session, 0), PreviewLoadStatus::Idle);
        assert_eq!(
            session.preview_status(LinkId(0), true),
            PreviewLoadStatus::Ready
        );
    }

    #[test]
    fn mermaid_diagram_index_parses_bmd_url() {
        assert_eq!(mermaid_diagram_index("bmd:mermaid:3"), Some(3));
        assert_eq!(mermaid_diagram_index("https://x"), None);
    }
}
