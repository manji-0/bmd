//! Shared preview-load task states used by mermaid and markdown image pipelines.

use super::document_generation::DocumentGeneration;
use super::link::LinkId;

/// Per-link preview load phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewLoadPhase {
    Idle,
    Queued,
    Loading,
    Ready,
    Failed,
}

/// Lifecycle state of one preview link's terminal image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewLoadTask {
    Idle,
    Queued,
    Loading { generation: DocumentGeneration },
    Ready,
    Failed(String),
}

impl PreviewLoadTask {
    pub fn phase(&self) -> PreviewLoadPhase {
        match self {
            Self::Idle => PreviewLoadPhase::Idle,
            Self::Queued => PreviewLoadPhase::Queued,
            Self::Loading { .. } => PreviewLoadPhase::Loading,
            Self::Ready => PreviewLoadPhase::Ready,
            Self::Failed(_) => PreviewLoadPhase::Failed,
        }
    }

    pub fn is_in_flight(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }
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

pub(crate) fn preview_status(
    tasks: &std::collections::HashMap<LinkId, PreviewLoadTask>,
    link_id: LinkId,
    cached: bool,
) -> PreviewLoadStatus {
    if cached {
        return PreviewLoadStatus::Ready;
    }
    match tasks.get(&link_id) {
        Some(PreviewLoadTask::Queued) => PreviewLoadStatus::Queued,
        Some(PreviewLoadTask::Loading { .. }) => PreviewLoadStatus::Loading,
        Some(PreviewLoadTask::Failed(_)) => PreviewLoadStatus::Failed,
        Some(PreviewLoadTask::Ready) => PreviewLoadStatus::Ready,
        Some(PreviewLoadTask::Idle) | None => PreviewLoadStatus::Idle,
    }
}
