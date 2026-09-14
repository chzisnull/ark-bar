use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::provider_models::ProviderUsageData;

pub struct UsageCache {
    inner: Mutex<Option<(Instant, ProviderUsageData)>>,
    refreshing: AtomicBool,
}

impl UsageCache {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            refreshing: AtomicBool::new(false),
        }
    }

    pub fn get_fresh(&self, ttl: Duration) -> Option<ProviderUsageData> {
        let guard = self.inner.lock().ok()?;
        let (cached_at, data) = guard.as_ref()?;
        if cached_at.elapsed() < ttl {
            Some(data.clone())
        } else {
            None
        }
    }

    pub fn get_stale(&self, max_age: Duration) -> Option<ProviderUsageData> {
        let guard = self.inner.lock().ok()?;
        let (cached_at, data) = guard.as_ref()?;
        if cached_at.elapsed() < max_age {
            Some(data.clone())
        } else {
            None
        }
    }

    pub fn peek(&self) -> Option<ProviderUsageData> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|(_, d)| d.clone()))
    }

    pub fn set(&self, data: ProviderUsageData) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some((Instant::now(), data));
        }
        self.refreshing.store(false, Ordering::SeqCst);
    }

    pub fn begin_refresh(&self) -> bool {
        self.refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn end_refresh(&self) {
        self.refreshing.store(false, Ordering::SeqCst);
    }
}
