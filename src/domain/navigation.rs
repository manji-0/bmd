//! Layered back/reset navigation: anchor stack before document stack.
//!
//! Back (`O`) and reset (`Esc`) always consult the anchor stack first. Document
//! navigation applies only after the anchor stack top layer is current (no pending
//! anchor link jumps).

use super::NavStack;

/// Witness that the anchor stack top layer is the current section.
///
/// Document-stack steps require this proof so callers cannot skip anchor
/// link-jump consumption accidentally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorIdle;

impl AnchorIdle {
    /// Returns a witness when no prior anchor link jumps are stored.
    pub fn from_stack(stack: &NavStack) -> Option<Self> {
        stack.is_empty().then_some(Self)
    }
}

/// Active navigation layer for the next back/reset command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavLayer {
    /// Consume or reset in-document anchor jumps.
    Anchor,
    /// Pop or reset the nested document stack.
    Document,
}

/// Planned back (`O`) step before mutating application state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavBackPlan {
    AnchorStep,
    DocumentStep,
    Idle,
}

/// Planned reset (`Esc`) step before mutating application state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavResetPlan {
    AnchorReset,
    DocumentReset,
    Idle,
}

/// Resolve which layer handles the next back command.
pub fn plan_back(anchor: &NavStack, document_depth: usize) -> NavBackPlan {
    match active_layer(anchor) {
        NavLayer::Anchor => NavBackPlan::AnchorStep,
        NavLayer::Document if document_depth > 0 => NavBackPlan::DocumentStep,
        NavLayer::Document => NavBackPlan::Idle,
    }
}

/// Resolve which layer handles the next reset command.
pub fn plan_reset(anchor: &NavStack, document_depth: usize) -> NavResetPlan {
    match active_layer(anchor) {
        NavLayer::Anchor => NavResetPlan::AnchorReset,
        NavLayer::Document if document_depth > 0 => NavResetPlan::DocumentReset,
        NavLayer::Document => NavResetPlan::Idle,
    }
}

/// Document back is valid only with an idle anchor stack and a non-empty file stack.
pub fn plan_document_back(idle: AnchorIdle, document_depth: usize) -> Option<()> {
    let _ = idle;
    (document_depth > 0).then_some(())
}

/// Document reset is valid only with an idle anchor stack and a non-empty file stack.
pub fn plan_document_reset(idle: AnchorIdle, document_depth: usize) -> Option<()> {
    let _ = idle;
    (document_depth > 0).then_some(())
}

fn active_layer(anchor: &NavStack) -> NavLayer {
    if anchor.is_empty() {
        NavLayer::Document
    } else {
        NavLayer::Anchor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FixedScrollPrior;

    #[test]
    fn anchor_layer_blocks_document_back() {
        let mut anchor = NavStack::default();
        anchor
            .fix_prior_on_link_jump(FixedScrollPrior::fix(0))
            .unwrap();
        assert_eq!(plan_back(&anchor, 3), NavBackPlan::AnchorStep);
        assert_eq!(plan_reset(&anchor, 3), NavResetPlan::AnchorReset);
        assert!(AnchorIdle::from_stack(&anchor).is_none());
    }

    #[test]
    fn document_layer_requires_idle_anchor() {
        let anchor = NavStack::default();
        let idle = AnchorIdle::from_stack(&anchor).unwrap();
        assert_eq!(plan_back(&anchor, 2), NavBackPlan::DocumentStep);
        assert_eq!(plan_reset(&anchor, 2), NavResetPlan::DocumentReset);
        assert!(plan_document_back(idle, 2).is_some());
        assert!(plan_document_reset(idle, 2).is_some());
    }

    #[test]
    fn idle_when_both_stacks_empty() {
        let anchor = NavStack::default();
        assert_eq!(plan_back(&anchor, 0), NavBackPlan::Idle);
        assert_eq!(plan_reset(&anchor, 0), NavResetPlan::Idle);
    }

    #[test]
    fn draining_anchor_unlocks_document_reset() {
        let mut anchor = NavStack::default();
        anchor
            .fix_prior_on_link_jump(FixedScrollPrior::fix(5))
            .unwrap();
        assert_eq!(plan_reset(&anchor, 2), NavResetPlan::AnchorReset);
        let origin = anchor.step_reset().unwrap();
        assert_eq!(origin, 5);
        let idle = AnchorIdle::from_stack(&anchor).unwrap();
        assert_eq!(plan_reset(&anchor, 2), NavResetPlan::DocumentReset);
        assert!(plan_document_reset(idle, 2).is_some());
    }
}
