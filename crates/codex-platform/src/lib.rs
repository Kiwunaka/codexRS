use std::num::NonZeroUsize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePolicy {
    pub git_debounce: Duration,
    pub max_parallel_git_processes: NonZeroUsize,
    pub graceful_shutdown_timeout: Duration,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            git_debounce: Duration::from_millis(300),
            max_parallel_git_processes: NonZeroUsize::MIN,
            graceful_shutdown_timeout: Duration::from_secs(3),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePolicy;

    #[test]
    fn default_policy_prevents_parallel_git_storms() {
        assert_eq!(RuntimePolicy::default().max_parallel_git_processes.get(), 1);
    }
}
