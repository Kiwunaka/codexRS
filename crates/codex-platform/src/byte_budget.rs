use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct ByteBudget {
    inner: Arc<ByteBudgetInner>,
}

#[derive(Debug)]
struct ByteBudgetInner {
    limit: usize,
    used: Mutex<usize>,
    available: Condvar,
}

#[derive(Debug)]
pub struct ByteLease {
    inner: Arc<ByteBudgetInner>,
    bytes: usize,
}

impl ByteBudget {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(ByteBudgetInner {
                limit,
                used: Mutex::new(0),
                available: Condvar::new(),
            }),
        }
    }

    #[must_use]
    pub fn try_acquire(&self, bytes: usize) -> Option<ByteLease> {
        let mut used = lock_unpoisoned(&self.inner.used);
        self.acquire_if_available(&mut used, bytes)
    }

    #[must_use]
    pub fn acquire_timeout(&self, bytes: usize, timeout: Duration) -> Option<ByteLease> {
        if bytes > self.inner.limit {
            return None;
        }
        let deadline = Instant::now() + timeout;
        let mut used = lock_unpoisoned(&self.inner.used);
        loop {
            if let Some(lease) = self.acquire_if_available(&mut used, bytes) {
                return Some(lease);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            let waited = self.inner.available.wait_timeout(used, remaining);
            let (next_used, timed_out) = match waited {
                Ok((used, result)) => (used, result.timed_out()),
                Err(poisoned) => {
                    let (used, result) = poisoned.into_inner();
                    (used, result.timed_out())
                }
            };
            used = next_used;
            if timed_out && self.inner.limit.saturating_sub(*used) < bytes {
                return None;
            }
        }
    }

    fn acquire_if_available(
        &self,
        used: &mut MutexGuard<'_, usize>,
        bytes: usize,
    ) -> Option<ByteLease> {
        if bytes > self.inner.limit || self.inner.limit.saturating_sub(**used) < bytes {
            return None;
        }
        **used += bytes;
        Some(ByteLease {
            inner: Arc::clone(&self.inner),
            bytes,
        })
    }
}

impl Drop for ByteLease {
    fn drop(&mut self) {
        let mut used = lock_unpoisoned(&self.inner.used);
        *used = used.saturating_sub(self.bytes);
        drop(used);
        self.inner.available.notify_all();
    }
}

fn lock_unpoisoned(value: &Mutex<usize>) -> MutexGuard<'_, usize> {
    value
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::ByteBudget;

    #[test]
    fn dropping_a_lease_returns_its_bytes_to_the_budget() {
        let budget = ByteBudget::new(4);
        let Some(lease) = budget.try_acquire(4) else {
            panic!("first lease was not available");
        };
        assert!(budget.try_acquire(1).is_none());

        drop(lease);

        assert!(budget.try_acquire(4).is_some());
    }
}
