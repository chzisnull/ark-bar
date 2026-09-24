//! 搬运刘海时显示的四个落区 —— 按 Codenotch 的 `dropzones.rs` + `ui/dropzones.html` 移植。
//!
//! 一个透明、点击穿透的窗口盖住指针所在的那块屏幕，在四条边上各画出刘海自己的轮廓
//! （竖边是竖条、平边横过来，尺寸取刘海真实的长宽），指到哪条边哪条边就亮起来。
//! 页面是 `public/dropzones.html`；这一侧只负责窗口摆在哪、指针离哪条边最近。

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::notch::Screen;

pub const LABEL: &str = "dropzones";

/// 上一次推给页面的状态：页面可能在事件发出去之后才加载完，它会主动来要一次。
static CURRENT: Mutex<Option<Zones>> = Mutex::new(None);

#[derive(Clone, serde::Serialize)]
pub struct Zones {
    /// 覆盖窗口自己的逻辑尺寸
    pub w: f64,
    pub h: f64,
    /// 刘海贴着边时的深度与沿边长度（页面的 CSS px）
    pub depth: f64,
    pub length: f64,
    pub target: String,
}

/// 一个点属于哪条边：把屏幕按中线切成四个三角形取最近的边。
/// 用「最近」而不是命中测试那几条细带——落在 70px 的条里是穿针。
pub fn edge_at(x: f64, y: f64, w: f64, h: f64) -> &'static str {
    let (left, right, top, bottom) = (x, w - x, y, h - y);
    let nearest = left.min(right).min(top).min(bottom);
    if nearest == right {
        "right"
    } else if nearest == left {
        "left"
    } else if nearest == top {
        "top"
    } else {
        "bottom"
    }
}

/// 把覆盖层钉在 `screen` 的物理边界上（绝对坐标，跨屏也准）。
/// 窗口按逻辑尺寸建，再由物理像素摆正一次——换了缩放比的屏幕，平台可能按自己的理解换算。
fn pin(w: &tauri::WebviewWindow, screen: &Screen) {
    let size = tauri::PhysicalSize::new(screen.w.max(1) as u32, screen.h.max(1) as u32);
    let _ = w.set_position(tauri::PhysicalPosition::new(screen.x, screen.y));
    let _ = w.set_size(size);
    if w.outer_size().map(|s| s != size).unwrap_or(false) {
        let _ = w.set_position(tauri::PhysicalPosition::new(screen.x, screen.y));
        let _ = w.set_size(size);
    }
}

/// 现场建、用完销毁：一个一次用一秒钟的 WebView 不值得常驻一个进程。
pub fn show(app: &AppHandle, screen: &Screen, zones: &Zones) {
    if let Ok(mut cur) = CURRENT.lock() {
        *cur = Some(zones.clone());
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.emit_to(LABEL, "zones", zones);
        let _ = w.show();
        return;
    }
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("dropzones.html".into()))
        .title("ArkBar drop zones")
        .position(screen.x as f64 / screen.scale, screen.y as f64 / screen.scale)
        .inner_size(
            screen.w as f64 / screen.scale,
            screen.h as f64 / screen.scale,
        )
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        // 永不取焦点：搬运途中会在屏幕上重新出现，能取焦点的窗口会把焦点从用户手头抢走
        .focusable(false)
        .resizable(false);
    match builder.build() {
        Ok(w) => {
            pin(&w, screen);
            let _ = w.set_ignore_cursor_events(true);
            // 页面加载完会自己来要一次；这个覆盖相反的先后顺序
            let _ = w.emit_to(LABEL, "zones", zones);
            // 被搬运的是刘海，它该盖在它可能去的地方上面
            if let Some(notch) = app.get_webview_window("float") {
                let _ = notch.set_always_on_top(true);
            }
        }
        Err(e) => eprintln!("drop zones: {e}"),
    }
}

pub fn retarget(app: &AppHandle, zones: &Zones) {
    if let Ok(mut cur) = CURRENT.lock() {
        *cur = Some(zones.clone());
    }
    let _ = app.emit_to(LABEL, "zones", zones);
}

pub fn hide(app: &AppHandle) {
    if let Ok(mut cur) = CURRENT.lock() {
        *cur = None;
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}

/// 页面加载晚于第一次推送时，自己来要一份当前状态。
#[tauri::command]
pub fn get_zones() -> Option<Zones> {
    CURRENT.lock().ok().and_then(|g| g.clone())
}
