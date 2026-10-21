//! In-document anchor navigation as a link-jump stack.

use super::link_jump_stack::{LinkJumpStack, LinkJumpStackFull, PriorAtLinkJump};
use super::navigation_limits::new_anchor_link_stack;
use super::scroll_anchor::ScrollAnchor;

/// Scroll position fixed at the moment before an anchor link jump.
///
/// Stored as a [`ScrollAnchor`] so stepping back after a resize or outline
/// toggle lands on the same block.
pub type FixedScrollPrior = PriorAtLinkJump<ScrollAnchor>;

/// Anchor navigation stack: live scroll lives in view state.
///
/// [`FixedScrollPrior`] snapshots are stored only when following in-document anchor
/// links via [`NavStack::fix_prior_on_link_jump`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavStack(LinkJumpStack<ScrollAnchor>);

impl Default for NavStack {
    fn default() -> Self {
        Self(new_anchor_link_stack())
    }
}

/// No prior anchor link jumps are stored.
pub use super::link_jump_stack::LinkJumpStackEmpty as AnchorStackEmpty;

pub use super::navigation_limits::AnchorStackFull;

impl NavStack {
    /// Fix the current scroll offset and store it before following an anchor link.
    pub fn fix_prior_on_link_jump(
        &mut self,
        prior: FixedScrollPrior,
    ) -> Result<(), AnchorStackFull> {
        self.0
            .fix_prior_on_link_jump(prior)
            .map_err(|(LinkJumpStackFull, _)| AnchorStackFull)
    }

    pub fn step_back(&mut self) -> Result<ScrollAnchor, AnchorStackEmpty> {
        self.0.restore_latest_prior()
    }

    pub fn step_reset(&mut self) -> Result<ScrollAnchor, AnchorStackEmpty> {
        self.0.take_origin_prior()
    }

    pub fn clear(&mut self) {
        self.0.clear_priors();
    }

    #[cfg(test)]
    pub fn depth(&self) -> usize {
        self.0.fixed_prior_count()
    }

    #[cfg(test)]
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

    fn at(block: usize) -> ScrollAnchor {
        ScrollAnchor::new(block, 0, 1)
    }

    #[test]
    fn step_back_is_lifo() {
        let mut stack = NavStack::default();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(at(10)))
            .unwrap();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(at(20)))
            .unwrap();
        assert_eq!(stack.step_back(), Ok(at(20)));
        assert_eq!(stack.step_back(), Ok(at(10)));
        assert_eq!(stack.step_back(), Err(AnchorStackEmpty));
    }

    #[test]
    fn step_reset_returns_first_fixed_prior() {
        let mut stack = NavStack::default();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(at(10)))
            .unwrap();
        stack
            .fix_prior_on_link_jump(FixedScrollPrior::fix(at(20)))
            .unwrap();
        assert_eq!(stack.step_reset(), Ok(at(10)));
        assert!(stack.is_empty());
    }
}
