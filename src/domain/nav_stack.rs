//! In-document anchor navigation as a link-jump stack.

use super::link_jump_stack::{LinkJumpStack, LinkJumpStackFull, PriorAtLinkJump};
use super::navigation_limits::{ANCHOR_STACK_MAX_LAYERS, new_anchor_link_stack};

/// Scroll offset fixed at the moment before an anchor link jump.
pub type FixedScrollPrior = PriorAtLinkJump<usize>;

/// Anchor navigation stack: live scroll lives in view state.
///
/// [`FixedScrollPrior`] snapshots are stored only when following in-document anchor
/// links via [`NavStack::fix_prior_on_link_jump`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavStack(LinkJumpStack<usize>);

impl Default for NavStack {
    fn default() -> Self {
        Self(new_anchor_link_stack())
    }
}

/// No prior anchor link jumps are stored.
pub use super::link_jump_stack::LinkJumpStackEmpty as AnchorStackEmpty;

pub use super::navigation_limits::AnchorStackFull;

impl NavStack {
    pub fn max_layers() -> usize {
        ANCHOR_STACK_MAX_LAYERS
    }

    pub fn max_frames() -> usize {
        ANCHOR_STACK_MAX_LAYERS - 1
    }

    /// Fix the current scroll offset and store it before following an anchor link.
    pub fn fix_prior_on_link_jump(
        &mut self,
        prior: FixedScrollPrior,
    ) -> Result<(), AnchorStackFull> {
        self.0
            .fix_prior_on_link_jump(prior)
            .map_err(|LinkJumpStackFull| AnchorStackFull)
    }

    pub fn step_back(&mut self) -> Result<usize, AnchorStackEmpty> {
        self.0.restore_latest_prior()
    }

    pub fn step_reset(&mut self) -> Result<usize, AnchorStackEmpty> {
        self.0.reset_to_oldest_prior()
    }

    pub fn clear(&mut self) {
        self.0.clear_priors();
    }

    pub fn depth(&self) -> usize {
        self.0.fixed_prior_count()
    }

    pub fn current_layer(&self) -> usize {
        self.0.current_layer()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_at_origin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_back_is_lifo() {
        let mut stack = NavStack::default();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(20))
            .unwrap();
        assert_eq!(stack.step_back(), Ok(20));
        assert_eq!(stack.step_back(), Ok(10));
        assert_eq!(stack.step_back(), Err(AnchorStackEmpty));
    }

    #[test]
    fn step_reset_returns_first_fixed_prior() {
        let mut stack = NavStack::default();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(20))
            .unwrap();
        assert_eq!(stack.step_reset(), Ok(10));
        assert!(stack.is_empty());
    }
}
