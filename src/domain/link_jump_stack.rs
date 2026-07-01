//! Layered stack of positions fixed at link-jump time.
//!
//! The live current document/scroll position lives outside this stack. Each entry is
//! a snapshot taken once when the user follows a link; scrolling and other navigation
//! between jumps do not touch the stack.

/// State fixed at the moment before a link jump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorAtLinkJump<T>(T);

impl<T> PriorAtLinkJump<T> {
    /// Capture the current position/document at link-jump time.
    pub fn fix(value: T) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> T {
        self.0
    }

    pub fn as_inner(&self) -> &T {
        &self.0
    }
}

/// Stack is full; another link jump would exceed [`LinkJumpStack::max_layers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("link jump stack limit reached")]
pub struct LinkJumpStackFull;

/// No prior link-jump snapshots are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("link jump stack empty")]
pub struct LinkJumpStackEmpty;

/// Fixed prior states from link jumps. The live current item is not stored here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkJumpStack<T> {
    priors: Vec<PriorAtLinkJump<T>>,
    max_frames: usize,
}

impl<T> LinkJumpStack<T> {
    pub fn with_max_layers(max_layers: usize) -> Self {
        Self {
            priors: Vec::new(),
            max_frames: max_layers.saturating_sub(1),
        }
    }

    pub fn max_layers(&self) -> usize {
        self.max_frames + 1
    }

    pub fn max_frames(&self) -> usize {
        self.max_frames
    }

    /// Number of fixed priors. The live current layer is not included.
    pub fn fixed_prior_count(&self) -> usize {
        self.priors.len()
    }

    /// Active layer (1-based) including the live current item.
    pub fn current_layer(&self) -> usize {
        self.fixed_prior_count() + 1
    }

    pub fn is_at_origin(&self) -> bool {
        self.priors.is_empty()
    }

    /// Store the position/document fixed at this link jump.
    pub fn fix_prior_on_link_jump(
        &mut self,
        prior: PriorAtLinkJump<T>,
    ) -> Result<(), LinkJumpStackFull> {
        if self.fixed_prior_count() >= self.max_frames {
            return Err(LinkJumpStackFull);
        }
        self.priors.push(prior);
        Ok(())
    }

    /// Restore the most recent fixed prior (undo one link jump).
    pub fn restore_latest_prior(&mut self) -> Result<T, LinkJumpStackEmpty> {
        self.priors
            .pop()
            .map(PriorAtLinkJump::into_inner)
            .ok_or(LinkJumpStackEmpty)
    }

    pub fn oldest_prior(&self) -> Option<&T> {
        self.priors.first().map(PriorAtLinkJump::as_inner)
    }

    pub fn reset_to_oldest_prior(&mut self) -> Result<T, LinkJumpStackEmpty>
    where
        T: Clone,
    {
        let origin = self
            .priors
            .first()
            .map(|prior| prior.as_inner().clone())
            .ok_or(LinkJumpStackEmpty)?;
        self.priors.clear();
        Ok(origin)
    }

    /// Take the oldest fixed prior and clear the rest without cloning it.
    pub fn take_oldest_prior(&mut self) -> Result<T, LinkJumpStackEmpty> {
        if self.priors.is_empty() {
            return Err(LinkJumpStackEmpty);
        }
        let origin = PriorAtLinkJump::into_inner(self.priors.remove(0));
        self.priors.clear();
        Ok(origin)
    }

    pub fn clear_priors(&mut self) {
        self.priors.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_current_is_outside_stack_until_jump_fixes_prior() {
        let stack = LinkJumpStack::<usize>::with_max_layers(64);
        assert!(stack.is_at_origin());
        assert_eq!(stack.current_layer(), 1);
        assert_eq!(stack.fixed_prior_count(), 0);
    }

    #[test]
    fn only_link_jumps_add_fixed_priors() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        assert_eq!(stack.current_layer(), 2);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        assert_eq!(stack.current_layer(), 3);
        assert_eq!(stack.restore_latest_prior(), Ok(20));
        assert_eq!(stack.current_layer(), 2);
    }

    #[test]
    fn reset_returns_oldest_fixed_prior() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        assert_eq!(stack.reset_to_oldest_prior(), Ok(10));
        assert!(stack.is_at_origin());
    }

    #[test]
    fn take_oldest_prior_moves_without_clone() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        assert_eq!(stack.take_oldest_prior(), Ok(10));
        assert!(stack.is_at_origin());
    }

    #[test]
    fn rejects_jump_when_layer_limit_reached() {
        let mut stack = LinkJumpStack::with_max_layers(3);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(1))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(2))
            .unwrap();
        assert_eq!(stack.current_layer(), 3);
        assert_eq!(
            stack.fix_prior_on_link_jump(PriorAtLinkJump::fix(3)),
            Err(LinkJumpStackFull)
        );
    }
}
