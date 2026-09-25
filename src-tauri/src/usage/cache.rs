//! Shared plan-limit cache: one `T` per process, refreshed when its TTL is stale and keeping the
//! last known value whenever the undocumented endpoint fails.

use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Entry<T> {
    fetched_at: Instant,
    limits: T,
}

/// One cached plan-limits value. `T` is the limits struct of a single harness.
pub struct LimitsCache<T> {
    inner: Mutex<Option<Entry<T>>>,
}

impl<T: Clone> LimitsCache<T> {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Last known value, however old; `None` before the first successful fetch.
    pub fn last(&self) -> Option<T> {
        self.inner
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|entry| entry.limits.clone()))
    }

    /// Drops the stored value so the next `refresh` fetches again — used after switching accounts,
    /// when the cached limits belong to the account that just left.
    pub fn forget(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }

    /// Refreshes when the TTL is stale, keeping the last value when `fetch` fails.
    pub fn refresh(&self, ttl: Duration, fetch: impl FnOnce() -> Option<T>) -> Option<T> {
        if let Some(limits) = self.fresh(ttl) {
            return Some(limits);
        }

        match fetch() {
            Some(limits) => {
                self.store(limits.clone());
                Some(limits)
            }
            None => self.last(),
        }
    }

    fn store(&self, limits: T) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(Entry {
                fetched_at: Instant::now(),
                limits,
            });
        }
    }

    fn fresh(&self, ttl: Duration) -> Option<T> {
        self.inner.lock().ok().and_then(|guard| {
            guard
                .as_ref()
                .filter(|entry| entry.fetched_at.elapsed() < ttl)
                .map(|entry| entry.limits.clone())
        })
    }
}

impl<T: Clone> Default for LimitsCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn refresh_fetches_once_within_the_ttl() {
        let cache: LimitsCache<u32> = LimitsCache::new();
        let calls = Cell::new(0);
        let fetch = || {
            calls.set(calls.get() + 1);
            Some(7)
        };

        assert_eq!(cache.refresh(Duration::from_secs(300), fetch), Some(7));
        assert_eq!(cache.refresh(Duration::from_secs(300), fetch), Some(7));
        assert_eq!(calls.get(), 1, "second refresh is served from the cache");
    }

    #[test]
    fn refresh_keeps_the_last_value_when_fetch_fails() {
        let cache: LimitsCache<u32> = LimitsCache::new();
        assert_eq!(cache.refresh(Duration::ZERO, || None), None);

        assert_eq!(cache.refresh(Duration::from_secs(300), || Some(3)), Some(3));
        assert_eq!(cache.refresh(Duration::ZERO, || None), Some(3));
    }

    #[test]
    fn last_returns_the_stored_value() {
        let cache: LimitsCache<u32> = LimitsCache::new();
        assert_eq!(cache.last(), None);

        cache.refresh(Duration::from_secs(300), || Some(9));
        assert_eq!(cache.last(), Some(9));
    }

    #[test]
    fn forget_makes_the_next_refresh_fetch_again() {
        let cache: LimitsCache<u32> = LimitsCache::new();
        let calls = Cell::new(0);
        let fetch = || {
            calls.set(calls.get() + 1);
            Some(calls.get())
        };

        cache.refresh(Duration::from_secs(300), fetch);
        cache.forget();

        assert_eq!(cache.last(), None);
        assert_eq!(cache.refresh(Duration::from_secs(300), fetch), Some(2));
        assert_eq!(calls.get(), 2);
    }
}
