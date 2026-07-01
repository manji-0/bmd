//! Markdown image preview load lifecycle with typed state transitions.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::document_generation::DocumentGeneration;
use super::link::{LinkId, LinkKind};
use super::markdown::Document;
use super::preview_load::{
    PreviewLoadCompletionApplied, PreviewLoadStatus, PreviewLoadTask, preview_status,
};

/// Domain errors for image scheduling and completion.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImageRenderError {
    #[error("link {0} is not an image")]
    NotImage(LinkId),
    #[error("image source missing for {0}")]
    SourceMissing(LinkId),
    #[error("load failed: {0}")]
    Load(String),
}

/// Work item handed to the infrastructure layer for background loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSpawnRequest {
    pub link_id: LinkId,
    pub src: ImageSource,
    pub base_path: Option<PathBuf>,
    pub generation: DocumentGeneration,
}

/// Completion event from a background worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageCompletion {
    pub link_id: LinkId,
    pub generation: DocumentGeneration,
    pub outcome: Result<(), ImageRenderError>,
}

/// Image URL/path referenced by a markdown image link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSource(String);

impl ImageSource {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Serializable snapshot of image load progress for document navigation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ImageSessionSnapshot {
    generation: DocumentGeneration,
    tasks: HashMap<LinkId, PreviewLoadTask>,
    queue: VecDeque<LinkId>,
    queued: HashSet<LinkId>,
}

/// Aggregate image load state for the current document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRenderSession {
    generation: DocumentGeneration,
    tasks: HashMap<LinkId, PreviewLoadTask>,
    queue: VecDeque<LinkId>,
    queued: HashSet<LinkId>,
}

impl Default for ImageRenderSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageRenderSession {
    pub fn new() -> Self {
        Self {
            generation: DocumentGeneration::INITIAL,
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
        }
    }

    pub fn has_in_flight(&self) -> bool {
        self.tasks.values().any(|task| task.is_in_flight()) || !self.queue.is_empty()
    }

    pub fn preview_status(&self, link_id: LinkId, cached: bool) -> PreviewLoadStatus {
        preview_status(&self.tasks, link_id, cached)
    }

    pub fn begin_document(self) -> Self {
        Self {
            generation: self.generation.next(),
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
        }
    }

    pub fn schedule_prefetch(
        self,
        document: &Document,
        base_path: Option<&Path>,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<ImageSpawnRequest>) {
        let link_ids = document
            .links
            .iter()
            .enumerate()
            .filter(|(_, link)| link.kind == LinkKind::Image)
            .map(|(index, _)| LinkId(index))
            .collect::<Vec<_>>();
        self.schedule_visible_prefetch(&link_ids, document, base_path, is_cached)
    }

    /// Queue visible image links that are not already cached or in flight.
    pub fn schedule_visible_prefetch(
        mut self,
        visible: &[LinkId],
        document: &Document,
        base_path: Option<&Path>,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<ImageSpawnRequest>) {
        for &link_id in visible {
            let Some(link) = document.links.get(link_id.0) else {
                continue;
            };
            if link.kind != LinkKind::Image {
                continue;
            }
            if is_cached(link_id) {
                self = self.mark_ready(link_id);
                continue;
            }
            let _ = self.try_enqueue(link_id, document);
        }
        self.drain_spawns(document, base_path, MAX_CONCURRENT_IMAGE_LOADS)
    }

    pub fn request(
        mut self,
        link_id: LinkId,
        document: &Document,
        base_path: Option<&Path>,
        is_cached: bool,
    ) -> Result<(Self, Vec<ImageSpawnRequest>), ImageRenderError> {
        if is_cached {
            return Ok((self.mark_ready(link_id), Vec::new()));
        }
        self.try_enqueue(link_id, document)?;
        Ok(self.drain_spawns(document, base_path, MAX_CONCURRENT_IMAGE_LOADS))
    }

    pub fn apply_completion(
        mut self,
        completion: ImageCompletion,
        document: &Document,
        base_path: Option<&Path>,
    ) -> (Self, PreviewLoadCompletionApplied, Vec<ImageSpawnRequest>) {
        let applied = self.record_completion(completion);
        let (session, spawns) = if matches!(applied, PreviewLoadCompletionApplied::Stale) {
            (self, Vec::new())
        } else {
            self.drain_spawns(document, base_path, MAX_CONCURRENT_IMAGE_LOADS)
        };
        (session, applied, spawns)
    }

    pub fn suspend(self) -> ImageSessionSnapshot {
        let mut tasks = self.tasks;
        let mut queue = self.queue;
        let mut queued = self.queued;
        for (link_id, task) in &mut tasks {
            if matches!(task, PreviewLoadTask::Loading { .. }) {
                *task = PreviewLoadTask::Queued;
                if queued.insert(*link_id) {
                    queue.push_back(*link_id);
                }
            }
        }
        ImageSessionSnapshot {
            generation: self.generation,
            tasks,
            queue,
            queued,
        }
    }

    pub fn resume(
        snapshot: ImageSessionSnapshot,
        document: &Document,
        base_path: Option<&Path>,
        is_cached: impl Fn(LinkId) -> bool,
    ) -> (Self, Vec<ImageSpawnRequest>) {
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
        session.drain_spawns(document, base_path, MAX_CONCURRENT_IMAGE_LOADS)
    }

    fn mark_ready(mut self, link_id: LinkId) -> Self {
        self.tasks.insert(link_id, PreviewLoadTask::Ready);
        self.queued.remove(&link_id);
        self.queue.retain(|id| *id != link_id);
        self
    }

    fn try_enqueue(
        &mut self,
        link_id: LinkId,
        document: &Document,
    ) -> Result<(), ImageRenderError> {
        let _ = image_source_for_link(document, link_id)?;
        match self.tasks.get(&link_id) {
            Some(
                PreviewLoadTask::Queued | PreviewLoadTask::Loading { .. } | PreviewLoadTask::Ready,
            ) => return Ok(()),
            Some(PreviewLoadTask::Failed(_)) => {
                self.tasks.insert(link_id, PreviewLoadTask::Queued);
            }
            Some(PreviewLoadTask::Idle) | None => {
                self.tasks.insert(link_id, PreviewLoadTask::Queued);
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
        base_path: Option<&Path>,
        max_in_flight: usize,
    ) -> (Self, Vec<ImageSpawnRequest>) {
        let mut spawns = Vec::new();
        let in_flight = self.count_in_flight();
        let mut slots = max_in_flight.saturating_sub(in_flight);
        while slots > 0 {
            let Some(link_id) = self.queue.pop_front() else {
                break;
            };
            self.queued.remove(&link_id);
            if self.tasks.get(&link_id).is_some_and(|task| {
                matches!(
                    task,
                    PreviewLoadTask::Ready | PreviewLoadTask::Loading { .. }
                )
            }) {
                continue;
            }
            let Ok(src) = image_source_for_link(document, link_id) else {
                self.tasks.insert(
                    link_id,
                    PreviewLoadTask::Failed(ImageRenderError::SourceMissing(link_id).to_string()),
                );
                continue;
            };
            let generation = self.generation;
            self.tasks
                .insert(link_id, PreviewLoadTask::Loading { generation });
            spawns.push(ImageSpawnRequest {
                link_id,
                src,
                base_path: base_path.map(Path::to_path_buf),
                generation,
            });
            slots -= 1;
        }
        (self, spawns)
    }

    fn record_completion(&mut self, completion: ImageCompletion) -> PreviewLoadCompletionApplied {
        let Some(PreviewLoadTask::Loading { generation: active }) =
            self.tasks.get(&completion.link_id)
        else {
            return PreviewLoadCompletionApplied::Stale;
        };
        if completion.generation != *active {
            return PreviewLoadCompletionApplied::Stale;
        }
        match completion.outcome {
            Ok(()) => {
                self.tasks
                    .insert(completion.link_id, PreviewLoadTask::Ready);
                PreviewLoadCompletionApplied::Ready {
                    link_id: completion.link_id,
                }
            }
            Err(error) => {
                self.tasks.insert(
                    completion.link_id,
                    PreviewLoadTask::Failed(error.to_string()),
                );
                PreviewLoadCompletionApplied::Failed {
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
            if matches!(self.tasks.get(&link_id), Some(PreviewLoadTask::Queued))
                && self.queued.insert(link_id)
            {
                self.queue.push_back(link_id);
            }
        }
    }
}

/// Resolve image source URL for a link in `document`.
pub fn image_source_for_link(
    document: &Document,
    link_id: LinkId,
) -> Result<ImageSource, ImageRenderError> {
    let link = document
        .links
        .get(link_id.0)
        .ok_or(ImageRenderError::SourceMissing(link_id))?;
    if link.kind != LinkKind::Image {
        return Err(ImageRenderError::NotImage(link_id));
    }
    Ok(ImageSource(link.url.as_str().to_string()))
}

const MAX_CONCURRENT_IMAGE_LOADS: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Link, LinkUrl, PreviewLoadPhase};

    fn image_document(count: usize) -> Document {
        let links = (0..count)
            .map(|index| Link {
                url: LinkUrl::new(format!("assets/{index}.png")).unwrap(),
                title: None,
                kind: LinkKind::Image,
            })
            .collect();
        Document {
            blocks: vec![],
            links,
            mermaid_diagrams: vec![],
        }
    }

    #[test]
    fn visible_prefetch_only_queues_visible_images() {
        let document = image_document(3);
        let session = ImageRenderSession::new();
        let (session, spawns) =
            session.schedule_visible_prefetch(&[LinkId(1)], &document, None, |_| false);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].link_id, LinkId(1));
        assert!(matches!(
            session.tasks[&LinkId(1)],
            PreviewLoadTask::Loading { .. }
        ));
        assert!(!session.tasks.contains_key(&LinkId(0)));
    }

    #[test]
    fn request_queues_and_spawns() {
        let document = image_document(1);
        let session = ImageRenderSession::new();
        let (session, spawns) = session.request(LinkId(0), &document, None, false).unwrap();
        assert_eq!(spawns.len(), 1);
        assert!(matches!(
            session.tasks[&LinkId(0)],
            PreviewLoadTask::Loading { .. }
        ));
    }

    #[test]
    fn suspend_requeues_loading_tasks() {
        let document = image_document(1);
        let session = ImageRenderSession::new();
        let (session, _) = session.request(LinkId(0), &document, None, false).unwrap();
        let snapshot = session.suspend();
        assert_eq!(snapshot.tasks[&LinkId(0)].phase(), PreviewLoadPhase::Queued);
    }
}
