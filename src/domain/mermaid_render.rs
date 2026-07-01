//! Mermaid preview render lifecycle with typed state transitions.
//!
//! Each mermaid link progresses independently:
//! `Idle → Queued → Rendering → Ready | Failed`.
//!
//! [`DocumentGeneration`] invalidates in-flight work when the active document changes.

use std::collections::{HashMap, HashSet, VecDeque};

use super::document_generation::DocumentGeneration;
use super::link::{LinkId, LinkKind};
use super::markdown::Document;

/// Per-link mermaid render phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MermaidTaskPhase {
    Idle,
    Queued,
    Rendering,
    Ready,
    Failed,
}

/// Lifecycle state of one mermaid link's preview image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MermaidTask {
    Idle,
    Queued,
    Rendering { generation: DocumentGeneration },
    Ready,
    Failed(MermaidRenderError),
}

impl MermaidTask {
    pub fn phase(&self) -> MermaidTaskPhase {
        match self {
            Self::Idle => MermaidTaskPhase::Idle,
            Self::Queued => MermaidTaskPhase::Queued,
            Self::Rendering { .. } => MermaidTaskPhase::Rendering,
            Self::Ready => MermaidTaskPhase::Ready,
            Self::Failed(_) => MermaidTaskPhase::Failed,
        }
    }

    pub fn is_in_flight(&self) -> bool {
        matches!(self, Self::Rendering { .. })
    }
}

/// UI-facing preview status derived from task state and image cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MermaidPreviewStatus {
    /// No render has been requested for this link.
    Idle,
    /// Waiting for a worker slot.
    Queued,
    /// Background render in progress.
    Rendering,
    /// Terminal image is available in the render cache.
    Ready,
    /// Render failed; see [`MermaidRenderError`].
    Failed,
}

/// Domain errors for mermaid scheduling and completion.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MermaidRenderError {
    #[error("link {0} is not a mermaid diagram")]
    NotMermaid(LinkId),
    #[error("mermaid diagram not found for {0}")]
    DiagramMissing(LinkId),
    #[error("render failed: {0}")]
    Render(String),
}

/// Work item handed to the infrastructure layer for background rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidSpawnRequest {
    pub link_id: LinkId,
    pub source: MermaidSource,
    pub generation: DocumentGeneration,
}

/// Completion event from a background worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidCompletion {
    pub link_id: LinkId,
    pub generation: DocumentGeneration,
    pub outcome: Result<(), MermaidRenderError>,
}

/// Outcome applied to session state after accepting a completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MermaidCompletionApplied {
    Ready { link_id: LinkId },
    Failed { link_id: LinkId },
    Stale,
}

/// Non-empty mermaid diagram source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidSource(String);

impl MermaidSource {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Serializable snapshot of mermaid render progress for document navigation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MermaidSessionSnapshot {
    generation: DocumentGeneration,
    tasks: HashMap<LinkId, MermaidTask>,
    queue: VecDeque<LinkId>,
    queued: HashSet<LinkId>,
}

/// Aggregate mermaid render state for the current document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidRenderSession {
    generation: DocumentGeneration,
    tasks: HashMap<LinkId, MermaidTask>,
    queue: VecDeque<LinkId>,
    queued: HashSet<LinkId>,
}

impl Default for MermaidRenderSession {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidRenderSession {
    pub fn new() -> Self {
        Self {
            generation: DocumentGeneration::INITIAL,
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
        }
    }

    pub fn generation(&self) -> DocumentGeneration {
        self.generation
    }

    pub fn has_in_flight(&self) -> bool {
        self.tasks.values().any(|task| task.is_in_flight()) || !self.queue.is_empty()
    }

    pub fn preview_status(&self, link_id: LinkId, image_cached: bool) -> MermaidPreviewStatus {
        if image_cached {
            return MermaidPreviewStatus::Ready;
        }
        match self.tasks.get(&link_id) {
            Some(MermaidTask::Queued) => MermaidPreviewStatus::Queued,
            Some(MermaidTask::Rendering { .. }) => MermaidPreviewStatus::Rendering,
            Some(MermaidTask::Failed(_)) => MermaidPreviewStatus::Failed,
            Some(MermaidTask::Ready) => MermaidPreviewStatus::Ready,
            Some(MermaidTask::Idle) | None => MermaidPreviewStatus::Idle,
        }
    }

    /// Invalidate in-flight work and clear scheduling state for a new document.
    pub fn begin_document(self) -> Self {
        Self {
            generation: self.generation.next(),
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
        }
    }

    /// Queue every uncached mermaid link in document order.
    pub fn schedule_prefetch(
        mut self,
        document: &Document,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<MermaidSpawnRequest>) {
        for (index, link) in document.links.iter().enumerate() {
            if link.kind != LinkKind::Mermaid {
                continue;
            }
            let link_id = LinkId(index);
            if is_cached(link_id) {
                self = self.mark_ready(link_id);
                continue;
            }
            if let Ok(()) = self.try_enqueue(link_id, document) {
                // queued
            }
        }
        self.drain_spawns(document, MAX_CONCURRENT_MERMAID_RENDERS)
    }

    /// Queue visible mermaid links that are not already cached or in flight.
    pub fn schedule_visible_prefetch(
        mut self,
        visible: &[LinkId],
        document: &Document,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<MermaidSpawnRequest>) {
        for &link_id in visible {
            let Some(link) = document.links.get(link_id.0) else {
                continue;
            };
            if link.kind != LinkKind::Mermaid {
                continue;
            }
            if is_cached(link_id) {
                self = self.mark_ready(link_id);
                continue;
            }
            let _ = self.try_enqueue(link_id, document);
        }
        self.drain_spawns(document, MAX_CONCURRENT_MERMAID_RENDERS)
    }

    /// Queue one mermaid link, then start workers up to the concurrency limit.
    pub fn request(
        mut self,
        link_id: LinkId,
        document: &Document,
        is_cached: bool,
    ) -> Result<(Self, Vec<MermaidSpawnRequest>), MermaidRenderError> {
        if is_cached {
            return Ok((self.mark_ready(link_id), Vec::new()));
        }
        self.try_enqueue(link_id, document)?;
        Ok(self.drain_spawns(document, MAX_CONCURRENT_MERMAID_RENDERS))
    }

    /// Apply a worker completion and drain additional spawns if slots opened.
    pub fn apply_completion(
        mut self,
        completion: MermaidCompletion,
        document: &Document,
    ) -> (Self, MermaidCompletionApplied, Vec<MermaidSpawnRequest>) {
        let applied = self.record_completion(completion);
        let (session, spawns) = if matches!(applied, MermaidCompletionApplied::Stale) {
            (self, Vec::new())
        } else {
            self.drain_spawns(document, MAX_CONCURRENT_MERMAID_RENDERS)
        };
        (session, applied, spawns)
    }

    /// Re-queue in-flight tasks before pushing this document onto the navigation stack.
    pub fn suspend(self) -> MermaidSessionSnapshot {
        let mut tasks = self.tasks;
        let mut queue = self.queue;
        let mut queued = self.queued;
        for (link_id, task) in &mut tasks {
            if matches!(task, MermaidTask::Rendering { .. }) {
                *task = MermaidTask::Queued;
                if queued.insert(*link_id) {
                    queue.push_back(*link_id);
                }
            }
        }
        MermaidSessionSnapshot {
            generation: self.generation,
            tasks,
            queue,
            queued,
        }
    }

    /// Restore session state after document navigation and resume background work.
    pub fn resume(
        snapshot: MermaidSessionSnapshot,
        document: &Document,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<MermaidSpawnRequest>) {
        let mut session = Self {
            generation: snapshot.generation,
            tasks: snapshot.tasks,
            queue: snapshot.queue,
            queued: snapshot.queued,
        };
        for link_id in session.tasks.keys().copied().collect::<Vec<_>>() {
            if is_cached(link_id) {
                session = session.mark_ready(link_id);
            }
        }
        session.reconcile_queue();
        session.drain_spawns(document, MAX_CONCURRENT_MERMAID_RENDERS)
    }

    pub fn snapshot(&self) -> MermaidSessionSnapshot {
        MermaidSessionSnapshot {
            generation: self.generation,
            tasks: self.tasks.clone(),
            queue: self.queue.clone(),
            queued: self.queued.clone(),
        }
    }

    fn mark_ready(mut self, link_id: LinkId) -> Self {
        self.tasks.insert(link_id, MermaidTask::Ready);
        self.queued.remove(&link_id);
        self.queue.retain(|id| *id != link_id);
        self
    }

    fn try_enqueue(
        &mut self,
        link_id: LinkId,
        document: &Document,
    ) -> Result<(), MermaidRenderError> {
        let _ = mermaid_source_for_link(document, link_id)?;
        match self.tasks.get(&link_id) {
            Some(MermaidTask::Queued | MermaidTask::Rendering { .. } | MermaidTask::Ready) => {
                return Ok(());
            }
            Some(MermaidTask::Failed(_)) => {
                self.tasks.insert(link_id, MermaidTask::Queued);
            }
            Some(MermaidTask::Idle) | None => {
                self.tasks.insert(link_id, MermaidTask::Queued);
            }
        }
        if self.queued.insert(link_id) {
            self.queue.push_back(link_id);
        }
        Ok(())
    }

    fn drain_spawns(
        mut self,
        document: &Document,
        max_in_flight: usize,
    ) -> (Self, Vec<MermaidSpawnRequest>) {
        let mut spawns = Vec::new();
        let in_flight = self.count_in_flight();
        let mut slots = max_in_flight.saturating_sub(in_flight);
        while slots > 0 {
            let Some(link_id) = self.queue.pop_front() else {
                break;
            };
            self.queued.remove(&link_id);
            if self.tasks.get(&link_id).is_some_and(|task| {
                matches!(task, MermaidTask::Ready | MermaidTask::Rendering { .. })
            }) {
                continue;
            }
            let Ok(source) = mermaid_source_for_link(document, link_id) else {
                self.tasks.insert(
                    link_id,
                    MermaidTask::Failed(MermaidRenderError::DiagramMissing(link_id)),
                );
                continue;
            };
            let generation = self.generation;
            self.tasks
                .insert(link_id, MermaidTask::Rendering { generation });
            spawns.push(MermaidSpawnRequest {
                link_id,
                source,
                generation,
            });
            slots -= 1;
        }
        (self, spawns)
    }

    fn record_completion(&mut self, completion: MermaidCompletion) -> MermaidCompletionApplied {
        let Some(MermaidTask::Rendering { generation: active }) =
            self.tasks.get(&completion.link_id)
        else {
            return MermaidCompletionApplied::Stale;
        };
        if completion.generation != *active {
            return MermaidCompletionApplied::Stale;
        }
        match completion.outcome {
            Ok(()) => {
                self.tasks.insert(completion.link_id, MermaidTask::Ready);
                MermaidCompletionApplied::Ready {
                    link_id: completion.link_id,
                }
            }
            Err(error) => {
                self.tasks
                    .insert(completion.link_id, MermaidTask::Failed(error.clone()));
                MermaidCompletionApplied::Failed {
                    link_id: completion.link_id,
                }
            }
        }
    }

    fn count_in_flight(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.is_in_flight())
            .count()
    }

    fn reconcile_queue(&mut self) {
        for link_id in self.tasks.keys().copied().collect::<Vec<_>>() {
            if matches!(self.tasks.get(&link_id), Some(MermaidTask::Queued))
                && self.queued.insert(link_id)
            {
                self.queue.push_back(link_id);
            }
        }
    }
}

/// Resolve mermaid source text for a link in `document`.
///
/// # Errors
///
/// Returns [`MermaidRenderError`] when the link is not mermaid or the diagram index is invalid.
pub fn mermaid_source_for_link(
    document: &Document,
    link_id: LinkId,
) -> Result<MermaidSource, MermaidRenderError> {
    let link = document
        .links
        .get(link_id.0)
        .ok_or(MermaidRenderError::DiagramMissing(link_id))?;
    if link.kind != LinkKind::Mermaid {
        return Err(MermaidRenderError::NotMermaid(link_id));
    }
    let diagram_idx = mermaid_diagram_index(link.url.as_str())
        .ok_or(MermaidRenderError::DiagramMissing(link_id))?;
    document
        .mermaid_diagrams
        .get(diagram_idx)
        .map(|diagram| MermaidSource(diagram.source.clone()))
        .ok_or(MermaidRenderError::DiagramMissing(link_id))
}

pub fn mermaid_diagram_index(url: &str) -> Option<usize> {
    url.strip_prefix("bmd:mermaid:")?.parse().ok()
}

const MAX_CONCURRENT_MERMAID_RENDERS: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Link, LinkUrl, MermaidDiagram};

    fn mermaid_document(count: usize) -> Document {
        let mut links = Vec::new();
        let mut diagrams = Vec::new();
        for index in 0..count {
            diagrams.push(MermaidDiagram {
                source: format!("graph TD; N{index};"),
            });
            links.push(Link {
                url: LinkUrl::new(format!("bmd:mermaid:{index}")).unwrap(),
                title: None,
                kind: LinkKind::Mermaid,
            });
        }
        Document {
            blocks: vec![],
            links,
            mermaid_diagrams: diagrams,
        }
    }

    #[test]
    fn idle_to_queued_to_rendering() {
        let document = mermaid_document(1);
        let session = MermaidRenderSession::new();
        let (session, spawns) = session.request(LinkId(0), &document, false).unwrap();
        assert_eq!(
            session.tasks[&LinkId(0)].phase(),
            MermaidTaskPhase::Rendering
        );
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].generation, DocumentGeneration::INITIAL);
    }

    #[test]
    fn stale_completion_is_ignored() {
        let document = mermaid_document(1);
        let session = MermaidRenderSession::new();
        let (session, _) = session.request(LinkId(0), &document, false).unwrap();
        let (session, applied, _) = session.apply_completion(
            MermaidCompletion {
                link_id: LinkId(0),
                generation: DocumentGeneration(99),
                outcome: Ok(()),
            },
            &document,
        );
        assert_eq!(applied, MermaidCompletionApplied::Stale);
        assert_eq!(
            session.tasks[&LinkId(0)].phase(),
            MermaidTaskPhase::Rendering
        );
    }

    #[test]
    fn completion_transitions_to_ready() {
        let document = mermaid_document(1);
        let session = MermaidRenderSession::new();
        let (session, spawns) = session.request(LinkId(0), &document, false).unwrap();
        let generation = spawns[0].generation;
        let (session, applied, _) = session.apply_completion(
            MermaidCompletion {
                link_id: LinkId(0),
                generation,
                outcome: Ok(()),
            },
            &document,
        );
        assert_eq!(
            applied,
            MermaidCompletionApplied::Ready { link_id: LinkId(0) }
        );
        assert_eq!(session.tasks[&LinkId(0)].phase(), MermaidTaskPhase::Ready);
    }

    #[test]
    fn begin_document_invalidates_generation() {
        let session = MermaidRenderSession::new().begin_document();
        assert_eq!(session.generation(), DocumentGeneration(1));
    }

    #[test]
    fn suspend_requeues_rendering_tasks() {
        let document = mermaid_document(1);
        let session = MermaidRenderSession::new();
        let (session, _) = session.request(LinkId(0), &document, false).unwrap();
        let snapshot = session.suspend();
        assert_eq!(snapshot.tasks[&LinkId(0)].phase(), MermaidTaskPhase::Queued);
        assert!(snapshot.queue.contains(&LinkId(0)));
    }

    #[test]
    fn resume_drains_after_navigation() {
        let document = mermaid_document(1);
        let session = MermaidRenderSession::new();
        let (session, _) = session.request(LinkId(0), &document, false).unwrap();
        let snapshot = session.suspend();
        let (session, spawns) = MermaidRenderSession::resume(snapshot, &document, |_| false);
        assert_eq!(spawns.len(), 1);
        assert_eq!(
            session.tasks[&LinkId(0)].phase(),
            MermaidTaskPhase::Rendering
        );
    }

    #[test]
    fn visible_prefetch_only_queues_visible_mermaid_links() {
        let document = mermaid_document(3);
        let session = MermaidRenderSession::new();
        let (session, spawns) =
            session.schedule_visible_prefetch(&[LinkId(1)], &document, |_| false);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].link_id, LinkId(1));
        assert_eq!(
            session.tasks[&LinkId(1)].phase(),
            MermaidTaskPhase::Rendering
        );
        assert!(!session.tasks.contains_key(&LinkId(0)));
    }

    #[test]
    fn prefetch_respects_cached_links() {
        let document = mermaid_document(2);
        let session = MermaidRenderSession::new();
        let (session, spawns) = session.schedule_prefetch(&document, |id| id.0 == 0);
        assert_eq!(session.tasks[&LinkId(0)].phase(), MermaidTaskPhase::Ready);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].link_id, LinkId(1));
    }

    #[test]
    fn preview_status_reflects_task_and_cache() {
        let session = MermaidRenderSession::new();
        assert_eq!(
            session.preview_status(LinkId(0), false),
            MermaidPreviewStatus::Idle
        );
        assert_eq!(
            session.preview_status(LinkId(0), true),
            MermaidPreviewStatus::Ready
        );
    }
}
