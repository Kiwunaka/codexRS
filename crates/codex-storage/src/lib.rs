use std::error::Error;
use std::fmt;

pub const MAX_INLINE_EVENT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_HISTORY_PAGE_SIZE: usize = 100;
pub const MAX_HISTORY_PAGE_SIZE: usize = 500;
pub const DIAGNOSTIC_LOG_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTooLarge {
    pub actual: usize,
    pub limit: usize,
}

impl fmt::Display for EventTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "inline event is {} bytes; the limit is {} bytes",
            self.actual, self.limit
        )
    }
}

impl Error for EventTooLarge {}

pub fn validate_inline_event_size(actual: usize) -> Result<(), EventTooLarge> {
    if actual <= MAX_INLINE_EVENT_BYTES {
        Ok(())
    } else {
        Err(EventTooLarge {
            actual,
            limit: MAX_INLINE_EVENT_BYTES,
        })
    }
}

#[must_use]
pub fn bounded_history_page_size(requested: usize) -> usize {
    requested.clamp(1, MAX_HISTORY_PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_HISTORY_PAGE_SIZE, MAX_HISTORY_PAGE_SIZE, MAX_INLINE_EVENT_BYTES,
        bounded_history_page_size, validate_inline_event_size,
    };

    #[test]
    fn rejects_the_observed_594_mb_failure_without_allocating_it() {
        let error = match validate_inline_event_size(594_127_437) {
            Err(error) => error,
            Ok(()) => panic!("the observed oversized event was accepted"),
        };
        assert_eq!(error.limit, MAX_INLINE_EVENT_BYTES);
        assert_eq!(error.actual, 594_127_437);
    }

    #[test]
    fn accepts_the_inline_limit_exactly() {
        assert!(validate_inline_event_size(MAX_INLINE_EVENT_BYTES).is_ok());
    }

    #[test]
    fn page_sizes_are_always_bounded() {
        assert_eq!(
            bounded_history_page_size(DEFAULT_HISTORY_PAGE_SIZE),
            DEFAULT_HISTORY_PAGE_SIZE
        );
        assert_eq!(bounded_history_page_size(0), 1);
        assert_eq!(bounded_history_page_size(usize::MAX), MAX_HISTORY_PAGE_SIZE);
    }
}
