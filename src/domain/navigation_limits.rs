//! Capacity limits for layered navigation stacks.

use super::link_jump_stack::LinkJumpStack;

/// Maximum anchor layers in one document, counting the current section as layer 1.
pub const ANCHOR_STACK_MAX_LAYERS: usize = 64;

/// Maximum prior scroll positions stored before anchor jumps.
pub const ANCHOR_STACK_MAX_FRAMES: usize = ANCHOR_STACK_MAX_LAYERS - 1;

/// Maximum document layers in a nested file chain, counting the root as layer 1.
pub const DOCUMENT_STACK_MAX_LAYERS: usize = 64;

/// Maximum prior-document frames stored on the file stack.
pub const DOCUMENT_STACK_MAX_FRAMES: usize = DOCUMENT_STACK_MAX_LAYERS - 1;

/// The anchor stack already holds the maximum number of prior scroll positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("anchor stack limit reached ({ANCHOR_STACK_MAX_LAYERS} levels)")]
pub struct AnchorStackFull;

/// The document stack already holds the maximum number of prior-document frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("document stack limit reached ({DOCUMENT_STACK_MAX_LAYERS} levels)")]
pub struct DocumentStackFull;

pub fn anchor_stack_limit_message() -> String {
    AnchorStackFull.to_string()
}

pub fn document_stack_limit_message() -> String {
    DocumentStackFull.to_string()
}

pub type AnchorLinkStack = LinkJumpStack<usize>;

pub fn new_anchor_link_stack() -> AnchorLinkStack {
    LinkJumpStack::with_max_layers(ANCHOR_STACK_MAX_LAYERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::link_jump_stack::{LinkJumpStackFull, PriorAtLinkJump};

    #[test]
    fn anchor_link_stack_counts_current_section_as_layer_one() {
        let stack = new_anchor_link_stack();
        assert_eq!(stack.current_layer(), 1);
        assert_eq!(stack.max_layers(), ANCHOR_STACK_MAX_LAYERS);
    }

    #[test]
    fn anchor_link_stack_capacity_is_64_layers_including_current_section() {
        let mut stack = new_anchor_link_stack();
        for i in 0..ANCHOR_STACK_MAX_FRAMES {
            assert!(
                stack
                    .fix_prior_on_link_jump(PriorAtLinkJump::fix(i))
                    .is_ok()
            );
        }
        assert_eq!(stack.current_layer(), ANCHOR_STACK_MAX_LAYERS);
        assert_eq!(
            stack.fix_prior_on_link_jump(PriorAtLinkJump::fix(0)),
            Err(LinkJumpStackFull)
        );
    }
}
