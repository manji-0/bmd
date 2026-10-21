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
    /// `max_layers` counts the live current item as layer 1. Values below 1
    /// are treated as 1 (origin only, no stored priors).
    pub fn with_max_layers(max_layers: usize) -> Self {
        let max_frames = max_layers.max(1).saturating_sub(1);
        Self {
            priors: Vec::with_capacity(max_frames),
            max_frames,
        }
    }

    /// Number of fixed priors. The live current layer is not included.
    pub fn fixed_prior_count(&self) -> usize {
        self.priors.len()
    }

    /// Active layer (1-based) including the live current item.
    #[cfg(test)]
    pub fn current_layer(&self) -> usize {
        self.fixed_prior_count() + 1
    }

    pub fn is_at_origin(&self) -> bool {
        self.priors.is_empty()
    }

    /// Store the position/document fixed at this link jump.
    ///
    /// On overflow the prior is returned so callers that moved unique state
    /// into it can put that state back.
    pub fn fix_prior_on_link_jump(
        &mut self,
        prior: PriorAtLinkJump<T>,
    ) -> Result<(), (LinkJumpStackFull, PriorAtLinkJump<T>)> {
        if self.fixed_prior_count() >= self.max_frames {
            return Err((LinkJumpStackFull, prior));
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

    /// Put back a prior just taken by [`Self::restore_latest_prior`], keeping
    /// every older prior. Used when restoring that prior failed.
    pub fn return_latest_prior(&mut self, value: T) {
        debug_assert!(
            self.priors.len() < self.max_frames,
            "returned prior exceeds stack capacity"
        );
        self.priors.push(PriorAtLinkJump::fix(value));
    }

    /// Priors other than the newest `keep`, oldest first.
    pub fn older_priors_mut(&mut self, keep: usize) -> impl Iterator<Item = &mut T> {
        let end = self.priors.len().saturating_sub(keep);
        self.priors[..end].iter_mut().map(|prior| &mut prior.0)
    }

    /// The newest `count` priors, newest first.
    pub fn newest_priors_mut(&mut self, count: usize) -> impl Iterator<Item = &mut T> {
        let start = self.priors.len().saturating_sub(count);
        self.priors[start..]
            .iter_mut()
            .rev()
            .map(|prior| &mut prior.0)
    }

    /// Every fixed prior, oldest first.
    #[cfg(test)]
    pub fn priors(&self) -> impl Iterator<Item = &T> {
        self.priors.iter().map(|prior| &prior.0)
    }

    /// Take every fixed prior, oldest first. The live current item stays outside.
    pub fn take_all_priors(&mut self) -> Vec<T> {
        std::mem::take(&mut self.priors)
            .into_iter()
            .map(PriorAtLinkJump::into_inner)
            .collect()
    }

    /// Replace the stored priors. Extra values beyond [`Self::max_frames`] are dropped
    /// (oldest kept). Callers should pass a list previously taken from this stack.
    pub fn restore_priors(&mut self, values: Vec<T>) {
        debug_assert!(
            values.len() <= self.max_frames,
            "restored prior count exceeds stack capacity"
        );
        self.priors = values
            .into_iter()
            .take(self.max_frames)
            .map(PriorAtLinkJump::fix)
            .collect();
    }

    /// Take the oldest (origin) prior and discard every newer prior.
    ///
    /// Used for reset-to-origin of Copy-sized stacks. Do not use this for
    /// nested-file restore — that must keep intermediate frames via
    /// [`Self::take_all_priors`].
    pub fn take_origin_prior(&mut self) -> Result<T, LinkJumpStackEmpty> {
        let mut iter = std::mem::take(&mut self.priors).into_iter();
        let origin = iter.next().ok_or(LinkJumpStackEmpty)?;
        Ok(origin.into_inner())
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
    fn take_origin_prior_discards_newer_priors() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        assert_eq!(stack.take_origin_prior(), Ok(10));
        assert!(stack.is_at_origin());
    }

    #[test]
    fn zero_max_layers_is_origin_only() {
        let mut stack = LinkJumpStack::with_max_layers(0);
        assert_eq!(
            stack.fix_prior_on_link_jump(PriorAtLinkJump::fix(1)),
            Err((LinkJumpStackFull, PriorAtLinkJump::fix(1)))
        );
    }

    #[test]
    fn take_all_priors_preserves_order_for_rollback() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        let taken = stack.take_all_priors();
        assert_eq!(taken, vec![10, 20]);
        assert!(stack.is_at_origin());
        stack.restore_priors(taken);
        assert_eq!(stack.fixed_prior_count(), 2);
        assert_eq!(stack.restore_latest_prior(), Ok(20));
        assert_eq!(stack.restore_latest_prior(), Ok(10));
    }

    #[test]
    fn return_latest_prior_keeps_older_priors() {
        let mut stack = LinkJumpStack::with_max_layers(4);
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(10))
            .unwrap();
        stack
            .fix_prior_on_link_jump(PriorAtLinkJump::fix(20))
            .unwrap();
        let latest = stack.restore_latest_prior().unwrap();
        stack.return_latest_prior(latest);
        assert_eq!(stack.restore_latest_prior(), Ok(20));
        assert_eq!(stack.restore_latest_prior(), Ok(10));
    }

    #[test]
    fn older_and_newest_priors_split_at_count() {
        let mut stack = LinkJumpStack::with_max_layers(5);
        for value in [1, 2, 3] {
            stack
                .fix_prior_on_link_jump(PriorAtLinkJump::fix(value))
                .unwrap();
        }
        let older: Vec<i32> = stack.older_priors_mut(2).map(|value| *value).collect();
        assert_eq!(older, vec![1]);
        assert_eq!(stack.older_priors_mut(5).count(), 0);
        let newest: Vec<i32> = stack.newest_priors_mut(2).map(|value| *value).collect();
        assert_eq!(newest, vec![3, 2]);
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
            Err((LinkJumpStackFull, PriorAtLinkJump::fix(3)))
        );
    }
}
