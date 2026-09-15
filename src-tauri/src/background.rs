// 后台用量同步线程。
// 旧实现把 setInterval 放在主窗口 webview 里，而主窗口平时是隐藏的
// popover——macOS 会挂起不可见 WKWebView 的 JS 定时器（配合 App Nap 甚
// 至挂起整个进程），Windows 的 WebView2 对隐藏窗口同样节流，导致设置
// 里的"后台自动检测频率"从不生效，托盘数字与面板数据只有点击托盘后才
// 刷新。改由 Rust 原生线程负责周期刷新，窗口隐藏时照常工作。
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::provider_manager::get_all_providers_usage;
use crate::tray;

/// 后台自动检测频率（分钟），0 = 仅手动刷新。
/// 与前端 localStorage 的 arkbar_refresh_interval 同步，默认 5 分钟。
static INTERVAL_MINUTES: AtomicU64 = AtomicU64::new(5);
/// 托盘标题跟随的厂商 id（"volcengine" | "grok" | ... | "auto"）。
static TRAY_TARGET: Mutex<String> = Mutex::new(String::new());
/// 菜单栏百分比显示模式：0=始终显示；1=仅告警(≥75%或断连)时显示；
/// 2=纯图标。与 App.vue 的 computeTrayTitle 语义保持一致。
static TRAY_PERCENT_MODE: AtomicU8 = AtomicU8::new(0);
/// 前端是否已把用户偏好同步过来。在此之前不刷新也不动托盘，
/// 避免按默认值覆盖用户的"仅手动/隐藏百分比"设置。
static PREFS_SYNCED: AtomicBool = AtomicBool::new(false);
/// "仅手动"切回自动模式时置位，触发立即补刷一轮。
static REFRESH_NOW: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_background_interval(minutes: u64) -> Result<(), String> {
    let old = INTERVAL_MINUTES.swap(minutes, Ordering::SeqCst);
    if old == 0 && minutes > 0 {
        REFRESH_NOW.store(true, Ordering::SeqCst);
    }
    PREFS_SYNCED.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn set_tray_prefs(target: String, percent_mode: String) -> Result<(), String> {
    if let Ok(mut t) = TRAY_TARGET.lock() {
        *t = target;
    }
    let mode = match percent_mode.as_str() {
        "alert" => 1,
        "never" => 2,
        _ => 0,
    };
    TRAY_PERCENT_MODE.store(mode, Ordering::SeqCst);
    PREFS_SYNCED.store(true, Ordering::SeqCst);
    Ok(())
}

/// 启动后台同步线程。以 5 秒为节拍轮询到点情况，
/// 用户在设置里改频率后无需重启即可生效。
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_run = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let minutes = INTERVAL_MINUTES.load(Ordering::SeqCst);
            if !PREFS_SYNCED.load(Ordering::SeqCst) || minutes == 0 {
                // 未同步偏好或仅手动模式：挂起计时，重新启用时立即补刷一轮
                continue;
            }
            if REFRESH_NOW.swap(false, Ordering::SeqCst) {
                last_run = Instant::now();
                refresh_cycle(app.clone());
                continue;
            }
            if last_run.elapsed() < Duration::from_secs(minutes * 60) {
                continue;
            }
            last_run = Instant::now();
            refresh_cycle(app.clone());
        }
    });
}

/// 一轮全量刷新：四家厂商并发拉取（内部各自走 UsageCache 降级链），
/// 逐家广播给前端，并按用户配置直接更新托盘标题。
///
/// 注意：托盘标题的目标回落（auto→volcengine）与格式必须与
/// App.vue 的 updateTrayTitle 保持一致，两处任一改动需同步。
fn refresh_cycle(app: AppHandle) {
    let target = {
        let t = TRAY_TARGET.lock().map(|g| g.clone()).unwrap_or_default();
        // 与前端 updateTrayTitle 的语义一致：auto 固定回落火山方舟
        if t.is_empty() || t == "auto" || t == "volcengine" {
            "volcengine".to_string()
        } else {
            t
        }
    };
    let percent_mode = TRAY_PERCENT_MODE.load(Ordering::SeqCst);

    for data in get_all_providers_usage() {
        if data.provider == target {
            let title = if percent_mode == 2 {
                String::new()
            } else if !data.is_connected {
                // 告警模式下断连要在菜单栏可见
                if percent_mode == 1 { " ⚠".to_string() } else { String::new() }
            } else {
                match data.primary_session_percent {
                    Some(p) if percent_mode == 0 || p >= 75.0 => format!(" {}%", p.round() as i64),
                    _ => String::new(),
                }
            };
            let _ = tray::update_tray_title(app.clone(), title);
        }
        // 广播给两个 webview：主面板与悬浮窗收到后更新本地状态
        let _ = app.emit("usage-updated", &data);
    }
}
