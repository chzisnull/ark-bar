use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::provider_models::ProviderUsageData;

/// 这类未连接是「用户能自己解决」的：没装 CLI、没登录、凭据失效。
/// 必须当场替换缓存并显示出来，不能拿上一次的旧读数盖过去。
fn is_actionable_failure(data: &ProviderUsageData) -> bool {
    const MARKERS: [&str; 6] = [
        "重新授权",
        "重新登录",
        "登录已失效",
        "未登录",
        "未检测到 arkcli",
        "未安装",
    ];
    let text = format!(
        "{} {}",
        data.status_message.as_deref().unwrap_or(""),
        data.error_message.as_deref().unwrap_or("")
    );
    MARKERS.iter().any(|m| text.contains(m))
}

pub struct UsageCache {    inner: Mutex<Option<(Instant, ProviderUsageData)>>,
    refreshing: AtomicBool,
    /// 最近一次同步失败的时间：成功写入时清除。命中缓存返回前据此打
    /// "同步失败"标，覆盖后台线程静默失败时 UI 无感知的路径
    last_failure: Mutex<Option<Instant>>,
}

impl UsageCache {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            refreshing: AtomicBool::new(false),
            last_failure: Mutex::new(None),
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
        if let Ok(mut f) = self.last_failure.lock() {
            *f = None;
        }
        self.refreshing.store(false, Ordering::SeqCst);
    }

    /// Keep the last successful payload if a refresh fails or times out.
    /// Returns true when the new payload was stored, false when the previous
    /// successful payload was kept (i.e. the caller is serving stale data).
    pub fn set_prefer_connected(&self, data: ProviderUsageData) -> bool {
        // A provider may have usable stale values while its latest request
        // failed. Do not refresh the cache timestamp with that stale payload,
        // otherwise a failed SSO/API call would look fresh forever.
        if data.status_message.as_deref().is_some_and(|s| s.contains("同步失败")) {
            self.refreshing.store(false, Ordering::SeqCst);
            if let Ok(mut f) = self.last_failure.lock() {
                *f = Some(Instant::now());
            }
            return false;
        }
        if !data.is_connected {
            // 用户能自己解决的问题（没装 CLI、没登录、凭据失效）必须**当场**报出来：
            // 保留旧数据只会让界面继续显示上一次的读数，把「要重新授权」盖住——
            // 用户看到的就是「检测没问题，但就是不行」。
            if is_actionable_failure(&data) {
                self.set(data);
                return true;
            }
            if let Some(old) = self.peek() {
                if old.is_connected {
                    self.refreshing.store(false, Ordering::SeqCst);
                    if let Ok(mut f) = self.last_failure.lock() {
                        *f = Some(Instant::now());
                    }
                    return false;
                }
            }
        }
        self.set(data);
        true
    }

    /// 缓存命中返回前调用：若上次成功之后发生过同步失败，
    /// 给副本打标（缓存本体保持原样，成功后自动恢复）
    fn mark_if_failed(&self, mut data: ProviderUsageData) -> ProviderUsageData {
        if let Ok(f) = self.last_failure.lock() {
            if f.is_some() {
                data.status_message = Some("同步失败，展示最近一次成功的数据".to_string());
            }
        }
        data
    }

    pub fn get_or_refresh<F>(&'static self, force: bool, fetch: F) -> ProviderUsageData
    where
        F: FnOnce() -> ProviderUsageData + Send + 'static,
    {
        const FRESH_TTL: Duration = Duration::from_secs(20);
        const STALE_TTL: Duration = Duration::from_secs(6 * 60);

        if !force {
            if let Some(fresh) = self.get_fresh(FRESH_TTL) {
                return self.mark_if_failed(fresh);
            }
            if let Some(stale) = self.get_stale(STALE_TTL) {
                if self.begin_refresh() {
                    std::thread::spawn(move || {
                        // 捕获 panic：保证 set/set_prefer_connected 未执行时
                        // 也复位单飞标志，refreshing 不会永久卡死
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(fetch)) {
                            Ok(data) => {
                                self.set_prefer_connected(data);
                            }
                            Err(_) => self.refreshing.store(false, Ordering::SeqCst),
                        }
                    });
                }
                return self.mark_if_failed(stale);
            }
        }

        let data = fetch();
        let stored_new = self.set_prefer_connected(data.clone());
        if stored_new {
            data
        } else {
            // 失败保旧：给返回副本打标，让前端如实展示"同步失败"，
            // 缓存本体保持原样（下次成功后自动恢复）
            let mut stale = self.peek().unwrap_or(data);
            stale.status_message = Some("同步失败，展示最近一次成功的数据".to_string());
            stale
        }
    }

    pub fn begin_refresh(&self) -> bool {
        self.refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}
