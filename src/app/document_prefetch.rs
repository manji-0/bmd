//! Background document read/parse worker adapter over [`DocumentPrefetchSession`].

use std::mem;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::domain::{
    Document, DocumentPrefetchCompletion, DocumentPrefetchError, DocumentPrefetchSession,
    DocumentPrefetchSessionSnapshot, DocumentPrefetchSpawnRequest,
};
use crate::parse::parse;

struct WorkerResult {
    completion: DocumentPrefetchCompletion,
}

/// Runs background document reads and applies domain state transitions.
pub(crate) struct DocumentPrefetchPool {
    session: DocumentPrefetchSession,
    receiver: Receiver<WorkerResult>,
    sender: Sender<WorkerResult>,
}

impl Default for DocumentPrefetchPool {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            session: DocumentPrefetchSession::new(),
            receiver,
            sender,
        }
    }
}

impl DocumentPrefetchPool {
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
        let ready = session.ready_path_set();
        let is_ready = |path: &Path| ready.contains(path);
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

    pub fn ready_document(&self, path: &Path) -> Option<Document> {
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
        thread::spawn(move || {
            let outcome = (|| {
                let content = std::fs::read_to_string(&path)
                    .map_err(|error| DocumentPrefetchError::Read(error.to_string()))?;
                parse(&content).map_err(|error| DocumentPrefetchError::Parse(error.to_string()))
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
