//! Monotonic generation token for preview render scopes.

/// Monotonic token for the active document preview render scope.
///
/// Completions tagged with an older generation are discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DocumentGeneration(pub u64);

impl DocumentGeneration {
    pub const INITIAL: Self = Self(0);

    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}
