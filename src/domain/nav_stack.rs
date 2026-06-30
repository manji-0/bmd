//! Scroll-position stack for in-document anchor navigation.

/// Stack of scroll offsets visited before each anchor jump.
///
/// The bottom entry is the position before the first anchor navigation.
/// `pop` returns the most recent prior position; `bottom` is the origin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavStack {
    frames: Vec<usize>,
}

impl NavStack {
    pub fn push(&mut self, scroll_offset: usize) {
        self.frames.push(scroll_offset);
    }

    pub fn pop(&mut self) -> Option<usize> {
        self.frames.pop()
    }

    pub fn bottom(&self) -> Option<usize> {
        self.frames.first().copied()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::NavStack;

    #[test]
    fn push_pop_lifo() {
        let mut stack = NavStack::default();
        stack.push(10);
        stack.push(20);
        assert_eq!(stack.pop(), Some(20));
        assert_eq!(stack.pop(), Some(10));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn bottom_is_first_push() {
        let mut stack = NavStack::default();
        stack.push(10);
        stack.push(20);
        assert_eq!(stack.bottom(), Some(10));
    }
}
