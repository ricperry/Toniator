use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const ACTIVE: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTED: u8 = 2;

/// Cooperative operation cancellation with an explicit commit boundary.
/// Cancellation can win only while an operation is active; after
/// `begin_commit` succeeds the externally-visible result is allowed to finish.
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicU8>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationCancelled;

impl fmt::Display for OperationCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation cancelled")
    }
}

impl std::error::Error for OperationCancelled {}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Returns false when the operation has already
    /// crossed its non-cancellable commit boundary.
    pub fn cancel(&self) -> bool {
        self.0
            .compare_exchange(ACTIVE, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == CANCELLED
    }

    pub fn checkpoint(&self) -> Result<(), OperationCancelled> {
        if self.is_cancelled() {
            Err(OperationCancelled)
        } else {
            Ok(())
        }
    }

    /// Atomically claims the externally-visible commit. If cancellation won
    /// first, callers must discard temporary output instead of committing it.
    pub fn begin_commit(&self) -> Result<(), OperationCancelled> {
        self.0
            .compare_exchange(ACTIVE, COMMITTED, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
            .map_err(|_| OperationCancelled)
    }

    pub fn is_committed(&self) -> bool {
        self.0.load(Ordering::SeqCst) == COMMITTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_commit_have_one_deterministic_winner() {
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(!token.cancel());
        assert_eq!(token.begin_commit(), Err(OperationCancelled));
        let token = CancellationToken::new();
        assert_eq!(token.begin_commit(), Ok(()));
        assert!(!token.cancel());
        assert_eq!(token.checkpoint(), Ok(()));
    }
}
