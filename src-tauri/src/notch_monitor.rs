use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Hot rectangles in **physical pixels**, window-relative, as [x, y, w, h]:
/// when folded, this is the wake band around the rest pill.
/// when expanded, this is the notch pill, the card, and the gap between them.
static HOT: Mutex<Vec<[f64; 4]>> = Mutex::new(Vec::new());

/// Whether the notch is currently expanded (reporting open pill + card)
static EXPANDED: AtomicBool = AtomicBool::new(false);

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
pub fn set_hot(rects: Vec<[f64; 4]>, expanded: bool) {
    if let Ok(mut lock) = HOT.lock() {
        *lock = rects;
    }
    EXPANDED.store(expanded, Ordering::Relaxed);
}

/// Spawns the native cursor watchdog thread.
/// Monitors global system cursor position against the notch window and hot rects.
/// Automatically toggles click-through (`set_ignore_cursor_events`) so clicks behind
/// the transparent window are never eaten, and emits `notch_pointer` (true/false)
/// and `notch_cursor` (logical x, y) for instantaneous hover reactions without requiring clicks!
pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut click_through: Option<bool> = None;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(WATCHDOG_MS));

            let Some(w) = app.get_webview_window("float") else {
                continue;
            };

            // If window is hidden, do nothing
            if !w.is_visible().unwrap_or(false) {
                continue;
            }

            let (Ok(win_pos), Ok(cur_pos)) = (w.outer_position(), app.cursor_position()) else {
                continue;
            };

            let rects = match HOT.lock() {
                Ok(r) => r.clone(),
                Err(_) => continue,
            };

            let lx = cur_pos.x - win_pos.x as f64;
            let ly = cur_pos.y - win_pos.y as f64;
            let size = w.outer_size().ok().map(|s| (s.width as f64, s.height as f64));

            let inside = cursor_in_hot(&rects, lx, ly, size);

            if click_through != Some(!inside) {
                let _ = w.set_ignore_cursor_events(!inside);
                click_through = Some(!inside);
                let _ = w.emit("notch_pointer", inside);
            }

            // Emit logical coordinates inside webview while cursor is inside hot region
            if inside {
                let scale = w.scale_factor().unwrap_or(1.0);
                let logical_x = lx / scale;
                let logical_y = ly / scale;
                let _ = w.emit("notch_cursor", (logical_x, logical_y));
            }
        }
    });
}
