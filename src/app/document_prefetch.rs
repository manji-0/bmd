//! Background document read/parse worker adapter over [`DocumentPrefetchSession`].

use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use crate::domain::{
    DocumentPrefetchCompletion, DocumentPrefetchError, DocumentPrefetchSession,
    DocumentPrefetchSessionSnapshot, DocumentPrefetchSpawnRequest, PrefetchedDocument,
};
use crate::parse::parse;

use super::worker_pool::WorkerPool;

struct WorkerResult {
    completion: DocumentPrefetchCompletion,
}

/// Runs background document reads and applies domain state transitions.
pub(crate) struct DocumentPrefetchPool {
    session: DocumentPrefetchSession,
    receiver: mpsc::Receiver<WorkerResult>,
    sender: mpsc::Sender<WorkerResult>,
    worker_pool: Arc<WorkerPool>,
}

impl DocumentPrefetchPool {
    pub(crate) fn new(worker_pool: Arc<WorkerPool>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            session: DocumentPrefetchSession::new(),
            receiver,
            sender,
            worker_pool,
        }
    }

    pub fn begin_document(&mut self) {
        let session = mem::take(&mut self.session);
        self.session = session.begin_document();
    }

    pub fn suspend(&self) -> DocumentPrefetchSessionSnapshot {
        self.session.clone().suspend()
    }

    pub fn resume(&mut self, snapshot: DocumentPrefetchSessionSnapshot) {
        let (session, spawns) = DocumentPrefetchSession::resume(snapshot);
        self.session = session;
        self.spawn_all(spawns);
    }

    pub fn prefetch_visible(
        &mut self,
        visible: &[crate::domain::LinkId],
        document: &crate::domain::Document,
        base_path: Option<&PathBuf>,
    ) {
        let session = mem::take(&mut self.session);
        let fresh_ready = session.fresh_ready_paths();
        let is_ready = move |path: &Path| fresh_ready.contains(path);
        let (session, spawns) = session.schedule_visible_prefetch(
            visible,
            document,
            base_path.map(PathBuf::as_path),
            is_ready,
        );
        self.session = session;
        self.spawn_all(spawns);
    }

    pub fn poll(&mut self) -> bool {
        let mut dirty = false;
        while let Ok(result) = self.receiver.try_recv() {
            let session = mem::take(&mut self.session);
            let (session, _, spawns) = session.apply_completion(result.completion);
            self.session = session;
            dirty = true;
            self.spawn_all(spawns);
        }
        dirty
    }

    pub fn ready_document(&self, path: &Path) -> Option<crate::domain::Document> {
        self.session.ready_document(path).cloned()
    }

    pub fn has_pending(&self) -> bool {
        self.session.has_in_flight()
    }

    fn spawn_all(&self, spawns: Vec<DocumentPrefetchSpawnRequest>) {
        for request in spawns {
            self.spawn_one(request);
        }
    }

    fn spawn_one(&self, request: DocumentPrefetchSpawnRequest) {
        let path = request.path;
        let sender = self.sender.clone();
        let generation = request.generation;
        let worker_pool = Arc::clone(&self.worker_pool);
        worker_pool.spawn(move || {
            let outcome = (|| -> Result<PrefetchedDocument, DocumentPrefetchError> {
                let mtime = std::fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .map_err(|error| DocumentPrefetchError::Read(error.to_string()))?;
                let content = std::fs::read_to_string(&path)
                    .map_err(|error| DocumentPrefetchError::Read(error.to_string()))?;
                let document = parse(&content)
                    .map_err(|error| DocumentPrefetchError::Parse(error.to_string()))?;
                Ok(PrefetchedDocument { document, mtime })
            })();
            let _ = sender.send(WorkerResult {
                completion: DocumentPrefetchCompletion {
                    path,
                    generation,
                    outcome,
                },
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use crate::domain::{Document, Link, LinkId, LinkKind, LinkUrl, normalize_document_path};
    use crate::parse::parse;

    use super::*;

    fn write_temp_markdown(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bmd-prefetch-pool-{name}-{}.md",
            std::process::id()
        ));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn ready_document_reflects_updated_mtime() {
        let path = normalize_document_path(write_temp_markdown("pool-mtime", "# v1\n"));
        let link_document = Document {
            blocks: vec![],
            links: vec![Link {
                url: LinkUrl::new(path.display().to_string()).unwrap(),
                title: None,
                kind: LinkKind::Document,
            }],
            mermaid_diagrams: vec![],
        };
        let mut pool = DocumentPrefetchPool::new(WorkerPool::new(1));
        pool.prefetch_visible(&[LinkId(0)], &link_document, None);
        while pool.poll() || pool.has_pending() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let first = pool.ready_document(&path).expect("prefetched document");
        assert_eq!(parse("# v1\n").unwrap().blocks.len(), first.blocks.len());

        std::thread::sleep(Duration::from_millis(1100));
        fs::write(&path, "# v2\nextra\n").unwrap();
        pool.prefetch_visible(&[LinkId(0)], &link_document, None);
        while pool.poll() || pool.has_pending() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let second = pool.ready_document(&path).expect("refreshed document");
        assert_eq!(
            second.blocks.len(),
            parse("# v2\nextra\n").unwrap().blocks.len()
        );
        let _ = fs::remove_file(path);
    }
}
