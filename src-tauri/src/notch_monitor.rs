use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// 每块屏的刘海各有各的热区（物理像素、窗口相对坐标，[x, y, w, h]）：
/// 收起时是唤醒带，展开时是刘海胶囊 + 气泡卡片 + 两者之间的缝。
/// 按窗口标签分开存——舰队里每个刘海都是独立的一块屏事实。
static HOT: Mutex<Vec<(String, Vec<[f64; 4]>)>> = Mutex::new(Vec::new());

/// 哪些刘海当前是展开态（上报的是展开热区）
static EXPANDED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Extra padding around hot rectangles in physical pixels for smooth boundary tracking
const HOT_PAD: f64 = 10.0;

/// Sampling interval for cursor position tracking in milliseconds
const WATCHDOG_MS: u64 = 35;

/// Check if cursor (lx, ly) relative to window top-left is inside any hot rect or the gap union
fn cursor_in_hot(rects: &[[f64; 4]], lx: f64, ly: f64, window_size: Option<(f64, f64)>) -> bool {
    if rects.is_empty() {
        return false;
    }
    if let Some((w, h)) = window_size {
        if lx < 0.0 || ly < 0.0 || lx > w || ly > h {
            return false;
        }
    }

    // Direct hit with slack padding
    for r in rects {
        if lx >= r[0] - HOT_PAD
            && ly >= r[1] - HOT_PAD
            && lx <= r[0] + r[2] + HOT_PAD
            && ly <= r[1] + r[3] + HOT_PAD
        {
            return true;
        }
    }

    // Gap between hot rectangles (e.g. between card and notch pill)
    if rects.len() > 1 {
        let x0 = rects.iter().map(|r| r[0]).fold(f64::MAX, f64::min);
        let y0 = rects.iter().map(|r| r[1]).fold(f64::MAX, f64::min);
        let x1 = rects.iter().map(|r| r[0] + r[2]).fold(f64::MIN, f64::max);
        let y1 = rects.iter().map(|r| r[1] + r[3]).fold(f64::MIN, f64::max);
        if lx >= x0 && ly >= y0 && lx <= x1 && ly <= y1 {
            return true;
        }
    }

    false
}

#[tauri::command]
pub fn set_hot(window: tauri::WebviewWindow, rects: Vec<[f64; 4]>, expanded: bool) {
    let label = window.label().to_string();
    if let Ok(mut lock) = HOT.lock() {
        lock.retain(|(l, _)| l != &label);
        lock.push((label.clone(), rects));
    }
    if let Ok(mut lock) = EXPANDED.lock() {
        lock.retain(|l| l != &label);
        if expanded {
            lock.push(label);
        }
    }
}

fn hot_for(label: &str) -> Vec<[f64; 4]> {
    HOT.lock()
        .ok()
        .and_then(|g| g.iter().find(|(l, _)| l == label).map(|(_, r)| r.clone()))
        .unwrap_or_default()
}

/// Spawns the native cursor watchdog thread.
/// Monitors global system cursor position against the notch window and hot rects.
/// Automatically toggles click-through (`set_ignore_cursor_events`) so clicks behind
/// the transparent window are never eaten, and emits `notch_pointer` (true/false)
/// and `notch_cursor` (logical x, y) for instantaneous hover reactions without requiring clicks!
pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        // 每个刘海窗口各自记住自己上一次的点穿状态（舰队里互不干扰）
        let mut click_through: Vec<(String, bool)> = Vec::new();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(WATCHDOG_MS));

            // 所有刘海窗口一起巡检：光标在哪块屏上，就只唤醒那一个
            let labels: Vec<String> = crate::notch::notch_labels(&app);
            for label in labels {
                let Some(w) = app.get_webview_window(&label) else {
                    continue;
                };
                // If window is hidden, do nothing
                if !w.is_visible().unwrap_or(false) {
                    continue;
                }
                let (Ok(win_pos), Ok(cur_pos)) = (w.outer_position(), app.cursor_position()) else {
                    continue;
                };

                let rects = hot_for(&label);
                let lx = cur_pos.x - win_pos.x as f64;
                let ly = cur_pos.y - win_pos.y as f64;
                let size = w.outer_size().ok().map(|s| (s.width as f64, s.height as f64));
                let inside = cursor_in_hot(&rects, lx, ly, size);

                let last = click_through.iter().find(|(l, _)| l == &label).map(|(_, v)| *v);
                if last != Some(!inside) {
                    let _ = w.set_ignore_cursor_events(!inside);
                    click_through.retain(|(l, _)| l != &label);
                    click_through.push((label.clone(), !inside));
                    // 事件只发给这一个窗口：广播会把每块屏的刘海一起展开
                    let _ = w.emit("notch_pointer", inside);
                    // 与上游同样的诊断：点穿状态每次翻转都记一行（含热区与光标相对位置），
                    // 「点了没反应」时第一个该看的就是它
                    eprintln!(
                        "ark-bar click-through {label} {} at cursor_rel=({lx:.0},{ly:.0}) rects={rects:?}",
                        if inside { "off (cursor on the notch)" } else { "on (cursor elsewhere)" }
                    );
                }

                // Emit logical coordinates inside webview while cursor is inside hot region
                if inside {
                    let scale = w.scale_factor().unwrap_or(1.0);
                    let logical_x = lx / scale;
                    let logical_y = ly / scale;
                    let _ = w.emit("notch_cursor", (logical_x, logical_y));
                }
            }
        }
    });
}
