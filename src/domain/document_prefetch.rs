//! Background prefetch lifecycle for local document links.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::document_generation::DocumentGeneration;
use super::document_link::{file_modified_time, normalize_document_path, resolve_document_path};
use super::link::{LinkId, LinkKind};
use super::markdown::Document;

/// Domain errors for document prefetch workers.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DocumentPrefetchError {
    #[error("read failed: {0}")]
    Read(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Work item handed to the infrastructure layer for background loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentPrefetchSpawnRequest {
    pub path: PathBuf,
    pub generation: DocumentGeneration,
}

/// Completion event from a background worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentPrefetchCompletion {
    pub path: PathBuf,
    pub generation: DocumentGeneration,
    pub outcome: Result<PrefetchedDocument, DocumentPrefetchError>,
}

/// Outcome applied to session state after accepting a completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentPrefetchCompletionApplied {
    Ready { path: PathBuf },
    Failed { path: PathBuf },
    Stale,
}

/// Parsed document kept ready for an instant link open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefetchedDocument {
    pub document: Document,
    pub mtime: SystemTime,
}

/// Serializable snapshot of document prefetch progress for document navigation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DocumentPrefetchSessionSnapshot {
    generation: DocumentGeneration,
    tasks: HashMap<PathBuf, DocumentPrefetchTask>,
    queue: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
    ready_order: VecDeque<PathBuf>,
}

/// Aggregate prefetch state for the current viewing document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentPrefetchSession {
    generation: DocumentGeneration,
    tasks: HashMap<PathBuf, DocumentPrefetchTask>,
    queue: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
    ready_order: VecDeque<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DocumentPrefetchTask {
    Queued,
    Loading { generation: DocumentGeneration },
    Ready(PrefetchedDocument),
    Failed(DocumentPrefetchError),
}

impl Default for DocumentPrefetchSession {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentPrefetchSession {
    pub fn new() -> Self {
        Self {
            generation: DocumentGeneration::INITIAL,
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
            ready_order: VecDeque::new(),
        }
    }

    pub fn has_in_flight(&self) -> bool {
        self.tasks.values().any(|task| task.is_in_flight()) || !self.queue.is_empty()
    }

    pub fn begin_document(self) -> Self {
        Self {
            generation: self.generation.next(),
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
            ready_order: VecDeque::new(),
        }
    }

    /// Queue visible document links that resolve to readable files.
    pub fn schedule_visible_prefetch(
        mut self,
        visible: &[LinkId],
        document: &Document,
        base_path: Option<&Path>,
        is_ready: impl Fn(&Path) -> bool,
    ) -> (Self, Vec<DocumentPrefetchSpawnRequest>) {
        self.invalidate_stale_entries();
        for &link_id in visible {
            let Some(link) = document.links.get(link_id.0) else {
                continue;
            };
            if link.kind != LinkKind::Document {
                continue;
            }
            let Ok(path) = resolve_document_path(base_path, link.url.as_str()) else {
                continue;
            };
            let path = normalize_document_path(path);
            if !path.is_file() {
                continue;
            }
            if is_ready(&path) {
                continue;
            }
            let _ = self.try_enqueue(path);
        }
        self.drain_spawns(MAX_CONCURRENT_DOCUMENT_LOADS)
    }

    pub fn apply_completion(
        mut self,
        completion: DocumentPrefetchCompletion,
    ) -> (
        Self,
        DocumentPrefetchCompletionApplied,
        Vec<DocumentPrefetchSpawnRequest>,
    ) {
        let applied = self.record_completion(completion);
        let (session, spawns) = if matches!(applied, DocumentPrefetchCompletionApplied::Stale) {
            (self, Vec::new())
        } else {
            self.drain_spawns(MAX_CONCURRENT_DOCUMENT_LOADS)
        };
        (session, applied, spawns)
    }

    pub fn ready_document(&self, path: &Path) -> Option<&Document> {
        let key = normalize_document_path(path.to_path_buf());
        match self.tasks.get(&key) {
            Some(DocumentPrefetchTask::Ready(prefetched))
                if prefetched_is_fresh(&key, prefetched) =>
            {
                Some(&prefetched.document)
            }
            _ => None,
        }
    }

    pub fn is_fresh_ready(&self, path: &Path) -> bool {
        let key = normalize_document_path(path.to_path_buf());
        self.tasks
            .get(&key)
            .is_some_and(|task| matches!(task, DocumentPrefetchTask::Ready(prefetched) if prefetched_is_fresh(&key, prefetched)))
    }

    pub fn fresh_ready_paths(&self) -> HashSet<PathBuf> {
        self.tasks
            .keys()
            .filter(|path| self.is_fresh_ready(path))
            .cloned()
            .collect()
    }

    pub fn invalidate_stale_entries(&mut self) {
        let stale = self
            .tasks
            .iter()
            .filter_map(|(path, task)| {
                if let DocumentPrefetchTask::Ready(prefetched) = task
                    && !prefetched_is_fresh(path, prefetched)
                {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for path in stale {
            self.tasks.remove(&path);
            self.ready_order.retain(|p| p != &path);
        }
    }

    pub fn ready_path_set(&self) -> HashSet<PathBuf> {
        self.tasks
            .iter()
            .filter(|(_, task)| matches!(task, DocumentPrefetchTask::Ready(_)))
            .map(|(path, _)| path.clone())
            .collect()
    }

    pub fn suspend(self) -> DocumentPrefetchSessionSnapshot {
        let mut tasks = self.tasks;
        let mut queue = self.queue;
        let mut queued = self.queued;
        for (path, task) in &mut tasks {
            if matches!(task, DocumentPrefetchTask::Loading { .. }) {
                *task = DocumentPrefetchTask::Queued;
                if queued.insert(path.clone()) {
                    queue.push_back(path.clone());
                }
            }
        }
        DocumentPrefetchSessionSnapshot {
            generation: self.generation,
            tasks,
            queue,
            queued,
            ready_order: self.ready_order,
        }
    }

    pub fn resume(
        snapshot: DocumentPrefetchSessionSnapshot,
    ) -> (Self, Vec<DocumentPrefetchSpawnRequest>) {
        let mut session = Self {
            generation: snapshot.generation,
            tasks: snapshot.tasks,
            queue: snapshot.queue,
            queued: snapshot.queued,
            ready_order: snapshot.ready_order,
        };
        session.reconcile_queue();
        session.drain_spawns(MAX_CONCURRENT_DOCUMENT_LOADS)
    }

    fn try_enqueue(&mut self, path: PathBuf) -> Result<(), ()> {
        match self.tasks.get(&path) {
            Some(DocumentPrefetchTask::Queued | DocumentPrefetchTask::Loading { .. }) => {
                return Ok(());
            }
            Some(DocumentPrefetchTask::Ready(_) | DocumentPrefetchTask::Failed(_)) => return Ok(()),
            None => {}
        }
        self.tasks
            .insert(path.clone(), DocumentPrefetchTask::Queued);
        if self.queued.insert(path.clone()) {
            self.queue.push_back(path);
        }
        Ok(())
    }

    fn drain_spawns(mut self, max_in_flight: usize) -> (Self, Vec<DocumentPrefetchSpawnRequest>) {
        let mut spawns = Vec::new();
        let in_flight = self.count_in_flight();
        let mut slots = max_in_flight.saturating_sub(in_flight);
        while slots > 0 {
            let Some(path) = self.queue.pop_front() else {
                break;
            };
            self.queued.remove(&path);
            if self.tasks.get(&path).is_some_and(|task| {
                matches!(
                    task,
                    DocumentPrefetchTask::Ready(_) | DocumentPrefetchTask::Loading { .. }
                )
            }) {
                continue;
            }
            let generation = self.generation;
            self.tasks
                .insert(path.clone(), DocumentPrefetchTask::Loading { generation });
            spawns.push(DocumentPrefetchSpawnRequest { path, generation });
            slots -= 1;
        }
        (self, spawns)
    }

    fn record_completion(
        &mut self,
        completion: DocumentPrefetchCompletion,
    ) -> DocumentPrefetchCompletionApplied {
        let path = normalize_document_path(completion.path);
        let Some(DocumentPrefetchTask::Loading { generation: active }) = self.tasks.get(&path)
        else {
            return DocumentPrefetchCompletionApplied::Stale;
        };
        if completion.generation != *active {
            return DocumentPrefetchCompletionApplied::Stale;
        }
        match completion.outcome {
            Ok(prefetched) => {
                self.store_ready(path.clone(), prefetched);
                DocumentPrefetchCompletionApplied::Ready { path }
            }
            Err(error) => {
                self.tasks
                    .insert(path.clone(), DocumentPrefetchTask::Failed(error));
                DocumentPrefetchCompletionApplied::Failed { path }
            }
        }
    }

    fn store_ready(&mut self, path: PathBuf, prefetched: PrefetchedDocument) {
        self.tasks
            .insert(path.clone(), DocumentPrefetchTask::Ready(prefetched));
        self.ready_order.retain(|p| p != &path);
        self.ready_order.push_back(path);
        self.evict_ready_if_needed();
    }

    fn evict_ready_if_needed(&mut self) {
        while self.count_ready() > MAX_PREFETCHED_DOCUMENTS {
            let Some(oldest) = self.ready_order.pop_front() else {
                break;
            };
            if matches!(
                self.tasks.get(&oldest),
                Some(DocumentPrefetchTask::Ready(_))
            ) {
                self.tasks.remove(&oldest);
            }
        }
    }

    fn count_ready(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| matches!(task, DocumentPrefetchTask::Ready(_)))
            .count()
    }

    fn count_in_flight(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.is_in_flight())
            .count()
    }

    fn reconcile_queue(&mut self) {
        for path in self.tasks.keys().cloned().collect::<Vec<_>>() {
            if matches!(self.tasks.get(&path), Some(DocumentPrefetchTask::Queued))
                && self.queued.insert(path.clone())
            {
                self.queue.push_back(path);
            }
        }
    }
}

impl DocumentPrefetchTask {
    fn is_in_flight(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }
}

fn prefetched_is_fresh(path: &Path, prefetched: &PrefetchedDocument) -> bool {
    file_modified_time(path).is_some_and(|mtime| mtime == prefetched.mtime)
}

const MAX_CONCURRENT_DOCUMENT_LOADS: usize = 2;
const MAX_PREFETCHED_DOCUMENTS: usize = 8;

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::domain::{Link, LinkUrl};

    fn write_temp_markdown(name: &str, content: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("bmd-prefetch-{name}-{}.md", std::process::id()));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn visible_prefetch_queues_resolved_files() {
        let path = normalize_document_path(write_temp_markdown("visible", "# Child\n"));
        let document = Document {
            blocks: vec![],
            links: vec![Link {
                url: LinkUrl::new(path.display().to_string()).unwrap(),
                title: None,
                kind: LinkKind::Document,
            }],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let session = DocumentPrefetchSession::new();
        let (session, spawns) =
            session.schedule_visible_prefetch(&[LinkId(0)], &document, None, |_| false);
        assert_eq!(spawns.len(), 1);
        assert!(matches!(
            session.tasks[&path],
            DocumentPrefetchTask::Loading { .. }
        ));
        let _ = fs::remove_file(path);
    }

    fn prefetched(document: Document, path: &Path) -> PrefetchedDocument {
        PrefetchedDocument {
            document,
            mtime: file_modified_time(path).expect("file mtime"),
        }
    }

    #[test]
    fn ready_document_survives_after_scroll_out() {
        let path = normalize_document_path(write_temp_markdown("ready", "# Child\n"));
        let document = Document {
            blocks: vec![],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let mut session = DocumentPrefetchSession::new();
        session.store_ready(path.clone(), prefetched(document.clone(), &path));
        assert_eq!(session.ready_document(&path), Some(&document));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn ready_cache_evicts_oldest_entries() {
        let mut session = DocumentPrefetchSession::new();
        let mut paths = Vec::new();
        for index in 0..=MAX_PREFETCHED_DOCUMENTS {
            let path = normalize_document_path(write_temp_markdown(
                &format!("evict-{index}"),
                "# Child\n",
            ));
            paths.push(path.clone());
            session.store_ready(
                path,
                prefetched(
                    Document {
                        blocks: vec![],
                        links: vec![],
                        mermaid_diagrams: vec![],
                        footnotes: vec![],
                        footnote_order: vec![],
                        front_matter: None,
                    },
                    &paths[index],
                ),
            );
        }
        assert_eq!(session.count_ready(), MAX_PREFETCHED_DOCUMENTS);
        let oldest = paths[0].clone();
        assert!(!session.tasks.contains_key(&oldest));
        for path in paths {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn completion_uses_typed_applied_variant() {
        let path = normalize_document_path(write_temp_markdown("done", "# Child\n"));
        let document = Document {
            blocks: vec![],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let mut session = DocumentPrefetchSession::new();
        session.tasks.insert(
            path.clone(),
            DocumentPrefetchTask::Loading {
                generation: DocumentGeneration::INITIAL,
            },
        );
        let (session, applied, _) = session.apply_completion(DocumentPrefetchCompletion {
            path: path.clone(),
            generation: DocumentGeneration::INITIAL,
            outcome: Ok(prefetched(document, &path)),
        });
        assert_eq!(
            applied,
            DocumentPrefetchCompletionApplied::Ready { path: path.clone() }
        );
        assert!(matches!(
            session.tasks[&path],
            DocumentPrefetchTask::Ready(_)
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn stale_ready_entries_are_invalidated_on_reschedule() {
        let path = normalize_document_path(write_temp_markdown("stale", "# v1\n"));
        let document = Document {
            blocks: vec![],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let mut session = DocumentPrefetchSession::new();
        session.store_ready(path.clone(), prefetched(document, &path));
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&path, "# v2\n").unwrap();
        let empty = Document {
            blocks: vec![],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let (session, spawns) = session.schedule_visible_prefetch(&[], &empty, None, |_| false);
        assert!(session.ready_document(&path).is_none());
        assert_eq!(spawns.len(), 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn suspend_and_resume_requeues_loading_tasks() {
        let path = normalize_document_path(PathBuf::from("/tmp/resume.md"));
        let mut session = DocumentPrefetchSession::new();
        session.tasks.insert(
            path.clone(),
            DocumentPrefetchTask::Loading {
                generation: DocumentGeneration::INITIAL,
            },
        );
        let snapshot = session.suspend();
        assert!(matches!(
            snapshot.tasks[&path],
            DocumentPrefetchTask::Queued
        ));
        let (session, spawns) = DocumentPrefetchSession::resume(snapshot);
        assert_eq!(spawns.len(), 1);
        assert!(matches!(
            session.tasks[&path],
            DocumentPrefetchTask::Loading { .. }
        ));
    }
}
