//! 刘海窗口的贴边摆放与拖动 —— 按 Codenotch 的 `NotchGeometry` / `place_notch` 移植。
//!
//! 上游（macOS 原版与 Rust/Tauri 移植版）有两条铁律，这里照抄：
//!
//! 1. **窗口永远焊在屏幕边缘。** 位置由「沿边比例 + 实测窗口尺寸」算出来
//!    （`edge_origin`），而不是把窗口挪到光标处。所以刘海不可能被拖到屏幕中间，
//!    也不可能离开它所属的那条边。
//! 2. **拖动只沿当前这条边滑动。** 按下抓手后由 Rust 线程跟随系统光标（WebView 的
//!    mousemove 在窗口自己开始移动后就不可靠了），只取沿边那一个轴的位移，换算成
//!    比例后重新摆放；松手即停，比例按边各自记住。
//!
//! 对齐物理屏幕边缘（`Monitor::position/size`），而不是工作区：Dock 或菜单栏的显隐
//! 不该让用户选好的位置动一下。

use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::dropzones;

/// 竖直贴边（右/左）时的窗口：宽度是刘海的深度 + 气泡卡片的空间，长度是厂商栈的长度。
const NOTCH_W: f64 = 440.0;
const NOTCH_LONG: f64 = 680.0;
/// 水平贴边（上/下）时横过来：一排厂商环 + 菲林 + 抓手约 504px，再留出下方气泡卡片的高度。
/// 高度取上游的方形尺寸（NOTCH_LONG=650）：横过来时卡片开在下方，520 就已经会把内容
/// 多一点的卡片顶出窗口（上游为此专门留了注释）。
const NOTCH_TOP_W: f64 = 700.0;
const NOTCH_TOP_H: f64 = 650.0;

/// 拖动跟随的采样间隔（上游同量级：8ms 足够跟手，又不至于把主线程喂满）
const DRAG_TICK_MS: u64 = 8;
/// 屏幕配置变化的巡检间隔
const WATCH_MS: u64 = 2000;

static NOTCH_EDGE: Mutex<String> = Mutex::new(String::new());
/// 覆盖范围（上游 NotchScreenScope）：main / all
static SCOPE: Mutex<String> = Mutex::new(String::new());
/// 沿边比例：0.5 = 正中。按边各自记（上游「只有当前这条边会动，其他边保留原处」）。
static ALONG_RIGHT: Mutex<f64> = Mutex::new(0.5);
static ALONG_TOP: Mutex<f64> = Mutex::new(0.5);
static LAST_ALONG_SENT: Mutex<f64> = Mutex::new(-1.0);
/// 状态文件位置（首次用到时确定）
static STATE_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// 贴边位置与沿边落点，落在磁盘上（上游的 `config.rs` 同款）。
/// 不靠 webview 的 localStorage：那是渲染层的东西，清了就丢，而且启动时
/// 页面还没挂载，Rust 已经要摆第一下了（否则会先摆右边缘再跳到顶部）。
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct NotchState {
    edge: String,
    along_right: f64,
    along_top: f64,
    /// "main" | "all"
    #[serde(default = "default_scope")]
    scope: String,
}

fn default_scope() -> String {
    "main".into()
}

impl Default for NotchState {
    fn default() -> Self {
        Self {
            edge: "right".into(),
            along_right: 0.5,
            along_top: 0.5,
            scope: "main".into(),
        }
    }
}

fn state_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    if let Ok(g) = STATE_PATH.lock() {
        if let Some(p) = g.as_ref() {
            return Some(p.clone());
        }
    }
    let dir = app.path().app_data_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("notch.json");
    if let Ok(mut g) = STATE_PATH.lock() {
        *g = Some(p.clone());
    }
    Some(p)
}

/// 启动时读一次：Rust 端第一下摆放就用用户上次选的那条边。
pub fn init_state(app: &AppHandle) {
    let Some(path) = state_file(app) else { return };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(s) = serde_json::from_str::<NotchState>(&text) else {
        return;
    };
    if let Ok(mut g) = NOTCH_EDGE.lock() {
        *g = edge_or_right(&s.edge).to_string();
    }
    if let Ok(mut g) = ALONG_RIGHT.lock() {
        *g = s.along_right.clamp(0.0, 1.0);
    }
    if let Ok(mut g) = ALONG_TOP.lock() {
        *g = s.along_top.clamp(0.0, 1.0);
    }
    if let Ok(mut g) = SCOPE.lock() {
        *g = if s.scope == "all" { "all".into() } else { "main".into() };
    }
}

fn save_state(app: &AppHandle) {
    let Some(path) = state_file(app) else { return };
    let s = NotchState {
        edge: current_edge(),
        along_right: ALONG_RIGHT.lock().map(|g| *g).unwrap_or(0.5),
        along_top: ALONG_TOP.lock().map(|g| *g).unwrap_or(0.5),
        scope: current_scope(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&s) {
        let _ = std::fs::write(path, text);
    }
}

/// 每块屏的刘海各拖各的（上游每块屏一个 model，hover 与拖动都是「每块屏的事实」）
static DRAGGING: Mutex<Vec<String>> = Mutex::new(Vec::new());
static DRAG_START: Mutex<Vec<(String, DragStart)>> = Mutex::new(Vec::new());

fn is_dragging(label: &str) -> bool {
    DRAGGING.lock().map(|g| g.iter().any(|l| l == label)).unwrap_or(false)
}

fn any_dragging() -> bool {
    DRAGGING.lock().map(|g| !g.is_empty()).unwrap_or(false)
}

fn set_dragging(label: &str, on: bool) {
    if let Ok(mut g) = DRAGGING.lock() {
        g.retain(|l| l != label);
        if on {
            g.push(label.to_string());
        }
    }
    if !on {
        if let Ok(mut g) = DRAG_START.lock() {
            g.retain(|(l, _)| l != label);
        }
    }
}

fn drag_start_for(label: &str) -> Option<DragStart> {
    DRAG_START
        .lock()
        .ok()
        .and_then(|g| g.iter().find(|(l, _)| l == label).map(|(_, d)| d.clone()))
}

fn set_drag_start(label: &str, start: DragStart) {
    if let Ok(mut g) = DRAG_START.lock() {
        g.retain(|(l, _)| l != label);
        g.push((label.to_string(), start));
    }
}

#[derive(Clone, Debug)]
struct DragStart {
    /// 按下时的全局光标位置（物理像素，左上原点）
    cursor: (f64, f64),
    /// 按下时的沿边比例
    ratio: f64,
    edge: String,
}

/// 一台显示器上摆放刘海需要知道的全部数字（上游 `Screen`）。
#[derive(Clone, Debug)]
pub struct Screen {
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub scale: f64,
}

impl Screen {
    pub fn of(m: &tauri::window::Monitor) -> Self {
        Self {
            name: m.name().cloned(),
            x: m.position().x,
            y: m.position().y,
            w: m.size().width as i32,
            h: m.size().height as i32,
            scale: m.scale_factor(),
        }
    }
}

pub fn edge_or_right(edge: &str) -> &str {
    if edge == "top" || edge == "bottom" || edge == "left" {
        edge
    } else {
        "right"
    }
}

fn is_vertical(edge: &str) -> bool {
    edge != "top" && edge != "bottom"
}

pub fn current_edge() -> String {
    NOTCH_EDGE
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "right".into())
}

fn along_slot(edge: &str) -> &'static Mutex<f64> {
    if is_vertical(edge) {
        &ALONG_RIGHT
    } else {
        &ALONG_TOP
    }
}

pub fn along(edge: &str) -> f64 {
    along_slot(edge).lock().map(|g| *g).unwrap_or(0.5)
}

fn set_along(edge: &str, ratio: f64) {
    if let Ok(mut g) = along_slot(edge).lock() {
        *g = ratio.clamp(0.0, 1.0);
    }
}

/// 每个边对应的窗口逻辑尺寸。
pub fn notch_window_size(edge: &str) -> (f64, f64) {
    if is_vertical(edge) {
        (NOTCH_W, NOTCH_LONG)
    } else {
        (NOTCH_TOP_W, NOTCH_TOP_H)
    }
}

/// 窗口左上角：`ratio` 是刘海沿边的**中心**比例，所以 0.5 永远是正中，与窗口多长无关。
/// 夹取到屏幕内，比例再离谱也跑不出这条边（上游 `edge_origin`）。
fn edge_origin(s: &Screen, edge: &str, ww: i32, wh: i32, ratio: f64) -> (i32, i32) {
    let along_of = |span: i32, len: i32| -> i32 {
        let v = (span as f64 * ratio - len as f64 / 2.0).round() as i32;
        v.clamp(0, (span - len).max(0))
    };
    match edge {
        "left" => (s.x, s.y + along_of(s.h, wh)),
        "top" => (s.x + along_of(s.w, ww), s.y),
        "bottom" => (s.x + along_of(s.w, ww), s.y + s.h - wh),
        _ => (s.x + s.w - ww, s.y + along_of(s.h, wh)),
    }
}

/// 刘海覆盖范围（上游 `NotchScreenScope`）：
/// `main` = 只有带菜单栏那一块屏（默认，和单面板时代一样）；`all` = 每块屏一个。
pub fn current_scope() -> String {
    SCOPE.lock().map(|g| g.clone()).unwrap_or_else(|_| "main".into())
}

/// 一块屏一个刘海窗口；标签固定为 float / float-1 / float-2…，与屏幕一一对应。
pub fn notch_label(index: usize) -> String {
    if index == 0 {
        "float".into()
    } else {
        format!("float-{index}")
    }
}

/// 现有刘海窗口的标签，按序号排好。
pub fn notch_labels(app: &AppHandle) -> Vec<String> {
    let mut labels: Vec<String> = app
        .webview_windows()
        .keys()
        .filter(|k| k.as_str() == "float" || k.starts_with("float-"))
        .cloned()
        .collect();
    labels.sort_by_key(|l| label_index(l));
    labels
}

fn label_index(label: &str) -> usize {
    label.strip_prefix("float-").and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// 现在应该在哪几块屏上各放一个刘海：主屏永远排第一，其余按位置排序，
/// 这样「第 i 个窗口 ↔ 第 i 块屏」的对应关系不会因枚举顺序变化而乱跳。
pub fn desired_screens(app: &AppHandle) -> Vec<Screen> {
    let Some(w) = app.get_webview_window("float").or_else(|| app.webview_windows().into_values().next()) else {
        return Vec::new();
    };
    let all: Vec<Screen> = w
        .available_monitors()
        .map(|ms| ms.iter().map(Screen::of).collect())
        .unwrap_or_default();
    let primary = w.primary_monitor().ok().flatten().map(|m| Screen::of(&m));
    plan_screens(&all, primary.as_ref(), &current_scope())
}

/// 该在哪几块屏上各放一个刘海（纯函数，好测）：主屏永远排第一，其余按位置排序，
/// 这样「第 i 个窗口 ↔ 第 i 块屏」的对应关系不会因枚举顺序变化而乱跳；
/// scope = main 时只留主屏那一块。
fn plan_screens(all: &[Screen], primary: Option<&Screen>, scope: &str) -> Vec<Screen> {
    let mut list: Vec<Screen> = all.to_vec();
    list.sort_by_key(|s| (s.x, s.y));
    if let Some(p) = primary {
        list.retain(|s| (s.x, s.y, s.w, s.h) != (p.x, p.y, p.w, p.h));
        list.insert(0, p.clone());
    }
    if scope == "all" {
        list
    } else {
        list.into_iter().take(1).collect()
    }
}

/// 第 i 个刘海该待的屏幕（窗口与屏幕按序号对应；屏幕少了就退回主屏）。
fn screen_for_index(app: &AppHandle, index: usize) -> Option<Screen> {
    let screens = desired_screens(app);
    screens.get(index).cloned().or_else(|| screens.first().cloned())
}

/// 某个刘海窗口所在的那块屏。
fn screen_for_window(app: &AppHandle, label: &str) -> Option<Screen> {
    screen_for_index(app, label_index(label))
}

/// 按覆盖范围把窗口补齐/收掉（上游 `NotchFleet.reconcile` 的等价物）。
pub fn reconcile_fleet(app: &AppHandle) {
    let desired = desired_screens(app);
    let labels: Vec<String> = (0..desired.len()).map(notch_label).collect();

    // 多出来的收掉（拔掉的显示器不留窗口）
    for (label, win) in app.webview_windows() {
        let is_notch = label == "float" || label.starts_with("float-");
        if is_notch && !labels.contains(&label) {
            let _ = win.destroy();
        }
    }
    // 缺的补上
    for label in &labels {
        if app.get_webview_window(label).is_none() {
            create_notch_window(app, label);
        }
    }
}

fn create_notch_window(app: &AppHandle, label: &str) {
    use tauri::WebviewUrl;
    let built = tauri::WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html#float".into()))
        .title("ArkBar Notch")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .visible(false)
        .disable_drag_drop_handler()
        .build();
    if let Err(e) = built {
        eprintln!("ark-bar 新建刘海窗口失败 {label}: {e}");
    }
}

/// 把窗口一次摆到位：尺寸与位置在**同一个**原生调用里设完。
///
/// 分两步（先 `set_size` 再 `set_position`）在 macOS 上必然出岔子：两次都投递到主线程，
/// 谁先落地不定；而且 AppKit 的 `setContentSize:` 保留窗口的底边，顶部就会差出一个
/// 高度差——换边时正好差 30pt（680 → 650），顶部刘海于是永远浮在菜单栏下方。
/// 上游同样是 `panel.setFrame(frame, display: true)` 一步到位。
#[cfg(target_os = "macos")]
fn set_notch_frame(app: &AppHandle, label: &str, main_h_points: f64, scale: f64, x: i32, y: i32, w: i32, h: i32) {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let s = if scale > 0.0 { scale } else { 1.0 };
    // AppKit 的全局坐标：原点是主屏左下角，y 向上；我们手上的 y 是从主屏顶部往下量的
    let rect = NSRect::new(
        NSPoint::new(x as f64 / s, main_h_points - (y as f64 + h as f64) / s),
        NSSize::new(w as f64 / s, h as f64 / s),
    );
    // AppKit 只能在主线程上碰：place_notch 会被拖动线程、命令线程调用，
    // 直接在那些线程上 setFrame 会 SIGTRAP（实测）。
    let handle = app.clone();
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || {
        let Some(win) = handle.get_webview_window(&label) else {
            return;
        };
        let Ok(ptr) = win.ns_window() else { return };
        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
        // 层级必须是 statusBar（25）而不是 alwaysOnTop 给的 floating（3）：
        // floating 的窗口会被 AppKit 压回菜单栏下方，顶部贴边永远差 30pt。
        // 上游的 NotchPanel 同样用 `level = .statusBar`。
        ns.setLevel(NSStatusWindowLevel);
        ns.setFrame_display(rect, true);
    });
}

#[cfg(not(target_os = "macos"))]
fn set_notch_frame(app: &AppHandle, label: &str, _main_h: f64, _scale: f64, x: i32, y: i32, w: i32, h: i32) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// 主屏高度（AppKit 点）——把左上原点的 y 换算到 AppKit 全局坐标要用它。
fn main_screen_height_points(window: &WebviewWindow) -> f64 {
    window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let s = m.scale_factor();
            m.size().height as f64 / if s > 0.0 { s } else { 1.0 }
        })
        .unwrap_or(0.0)
}

/// 把每一块屏上的刘海都钉到当前边、当前比例上。
pub fn place_notch(app: &AppHandle) {
    let labels = notch_labels(app);
    for (i, label) in labels.iter().enumerate() {
        let Some(w) = app.get_webview_window(label) else { continue };
        // 第 i 个窗口 ↔ 第 i 块屏（拔掉屏幕时退回主屏，不会留个窗口飘在不存在的地方）
        let Some(mon) = screen_for_index(app, i) else { continue };
        place_one(app, &w, &mon);
    }
    // 边与沿边比例是整队共享的：所有刘海一起翻布局、一起挪
    let edge = current_edge();
    let ratio = along(&edge);
    let _ = app.emit("notch_edge", &edge);
    if let Ok(mut last) = LAST_ALONG_SENT.lock() {
        if (*last - ratio).abs() > 1e-4 {
            *last = ratio;
            let _ = app.emit("notch_along", AlongPayload { edge: edge.clone(), along: ratio });
        }
    }
}

/// 摆一个刘海窗口。
fn place_one(app: &AppHandle, w: &WebviewWindow, mon: &Screen) {
    let label = w.label().to_string();
    let edge = current_edge();
    let (lw, lh) = notch_window_size(&edge);
    let target = PhysicalSize::new(
        (lw * mon.scale).round().max(1.0) as u32,
        (lh * mon.scale).round().max(1.0) as u32,
    );

    let current = w.outer_size().ok();
    let resized = current
        .map(|s| s.width != target.width || s.height != target.height)
        .unwrap_or(true);

    // 位置按目标尺寸算，然后连同尺寸一次设完（见 set_notch_frame 的注释）
    let ratio = along(&edge);
    let (x, y) = edge_origin(mon, &edge, target.width as i32, target.height as i32, ratio);
    let main_h = main_screen_height_points(w);
    set_notch_frame(
        app,
        &label,
        main_h,
        mon.scale,
        x,
        y,
        target.width as i32,
        target.height as i32,
    );

    // 平台若擅自改了尺寸（缩放换算、投递竞态），等它落地后按实际尺寸再摆一次。
    // 只在真的改过尺寸时才起线程：拖动时每 8ms 调一次 place_notch，这里不能有阻塞。
    if resized {
        let app2 = app.clone();
        let edge2 = edge.clone();
        let mon2 = mon.clone();
        let target2 = (target.width as i32, target.height as i32);
        let label2 = label.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            let Some(w) = app2.get_webview_window(&label2) else {
                return;
            };
            let Ok(actual) = w.outer_size() else { return };
            let (aw, ah) = (actual.width as i32, actual.height as i32);
            if (aw, ah) != target2 {
                let (x, y) = edge_origin(&mon2, &edge2, aw, ah, along(&edge2));
                let _ = w.set_position(PhysicalPosition::new(x, y));
            }
        });
    }

    // 摆放日志：刘海看不见时第一个该看的东西（上游同样每次都记一行）
    eprintln!(
        "ark-bar notch placed: win={label} edge={edge} pos=({x},{y}) size={}x{} mon={:?}=({},{} {}x{}) scale={} along={ratio:.3}",
        target.width, target.height, mon.name, mon.x, mon.y, mon.w, mon.h, mon.scale
    );
}

#[derive(Clone, serde::Serialize)]
struct AlongPayload {
    edge: String,
    along: f64,
}

/// 切换贴边位置。`along` 有值时用它恢复该边上次被拖到的位置（上游按边各自记住）。
#[tauri::command]
pub fn set_notch_edge(app: AppHandle, edge: String, along: Option<f64>) -> Result<(), String> {
    let edge = edge_or_right(&edge).to_string();
    if let Some(r) = along {
        set_along(&edge, r);
    }
    if let Ok(mut g) = NOTCH_EDGE.lock() {
        *g = edge;
    }
    save_state(&app);
    place_notch(&app);
    Ok(())
}

/// 切换覆盖范围：主显示器 / 所有显示器（上游 `NotchFleet.apply(scope:)`）。
#[tauri::command]
pub fn set_notch_scope(app: AppHandle, scope: String) -> Result<(), String> {
    if let Ok(mut g) = SCOPE.lock() {
        *g = if scope == "all" { "all".into() } else { "main".into() };
    }
    save_state(&app);
    reconcile_fleet(&app);
    place_notch(&app);
    Ok(())
}

#[tauri::command]
pub fn get_notch_scope() -> String {
    current_scope()
}

/// 复位居中：只动当前这条边（上游 `reset_bar` 的语义）。
#[tauri::command]
pub fn recentre_notch(app: AppHandle, edge: Option<String>) -> Result<(), String> {
    let edge = edge_or_right(&edge.unwrap_or_else(current_edge)).to_string();
    set_along(&edge, 0.5);
    if let Ok(mut g) = NOTCH_EDGE.lock() {
        *g = edge;
    }
    save_state(&app);
    place_notch(&app);
    Ok(())
}

#[cfg(target_os = "macos")]
fn left_button_down() -> bool {
    use objc2_app_kit::NSEvent;
    // 左键是否仍按着：拖动跟随线程靠它自己停下，不依赖页面回传 mouseup
    (NSEvent::pressedMouseButtons() & 1) != 0
}

#[cfg(not(target_os = "macos"))]
fn left_button_down() -> bool {
    // 非 macOS 上靠 end_notch_drag 收尾（循环同时看 DRAGGING，页面松手时会调它）
    true
}

/// 按住抓手开始**搬运**：落区升起，指针挑一条边，松手交付。
/// 刘海本身在松手前不动——选的是屏幕上的一块地方，不是移动了多远，所以什么都不跟手。
/// `depth` / `length` 是刘海竖起时自己的尺寸（页面的 CSS px）。
#[tauri::command]
pub fn begin_move(window: WebviewWindow, depth: f64, length: f64) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    if is_dragging(&label) {
        eprintln!("ark-bar notch carry ignored: {label} already dragging");
        return;
    }
    set_dragging(&label, true);
    eprintln!("ark-bar notch carry begin: win={label} depth={depth} length={length}");
    std::thread::spawn(move || {
        let lbl = label.clone();
        let done = move |app: &AppHandle| {
            dropzones::hide(app);
            set_dragging(&lbl, false);
            let _ = app.emit_to(&lbl, "move_end", ());
        };
        let Some(w) = app.get_webview_window(&label) else {
            done(&app);
            return;
        };
        let Some(start) = screen_for_window(&app, &label) else {
            done(&app);
            return;
        };
        let from = current_edge();
        let mon = start.clone();
        // 页面按覆盖窗口自己的逻辑尺寸画；刘海尺寸两边同一缩放比，直接给
        let zones_on = |s: &Screen, target: &str| dropzones::Zones {
            w: s.w as f64 / s.scale,
            h: s.h as f64 / s.scale,
            depth,
            length,
            target: target.to_string(),
        };
        let mut zones = zones_on(&mon, &from);
        dropzones::show(&app, &mon, &zones);
        let mut target = from.clone();

        while is_dragging(&label) && left_button_down() {
            if let Ok(cur) = app.cursor_position() {
                // 落区留在被搬的那个刘海自己那块屏上（上游 macOS 的 overlay 同样用
                // panel 所在屏的尺寸），指针跑到别的屏也不会把落区带过去——
                // 每块屏本来就已经各有一个刘海，换边是整队一起换。
                let next = dropzones::edge_at(
                    cur.x - mon.x as f64,
                    cur.y - mon.y as f64,
                    mon.w as f64,
                    mon.h as f64,
                );
                if next != target {
                    target = next.to_string();
                    zones.target = target.clone();
                    dropzones::retarget(&app, &zones);
                    let _ = app.emit("move_target", &target);
                }
            }
            std::thread::sleep(Duration::from_millis(16));
        }

        eprintln!("ark-bar notch carry: {from} -> {target} on {:?}", mon.name);
        if target != from {
            if let Ok(mut g) = NOTCH_EDGE.lock() {
                *g = target.clone();
            }
            save_state(&app);
            // 换边是整队的事：所有刘海一起翻到新边上（上游 apply(edge:) 的扇出）
            let _ = w;
            place_notch(&app);
        }
        done(&app);
    });
}

/// 按住抓手开始拖动。由 Rust 线程跟随系统光标，只沿当前边滑动。
#[tauri::command]
pub fn drag_begin(window: WebviewWindow) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    if is_dragging(&label) {
        return; // 已经在跟随了
    }
    eprintln!("ark-bar notch slide begin: win={label}");
    let edge = current_edge();
    let cursor = app.cursor_position().unwrap_or(PhysicalPosition::new(0.0, 0.0));
    set_drag_start(
        &label,
        DragStart {
            cursor: (cursor.x, cursor.y),
            ratio: along(&edge),
            edge: edge.clone(),
        },
    );
    set_dragging(&label, true);

    std::thread::spawn(move || {
        while is_dragging(&label) && left_button_down() {
            let Some(start) = drag_start_for(&label) else {
                break;
            };
            let Some(cur) = app.cursor_position().ok() else {
                break;
            };
            // 滑动要的是「这块屏上还剩多少可以走」的跨度，窗口本身不用碰
            if app.get_webview_window(&label).is_none() {
                break;
            }
            let Some(mon) = screen_for_window(&app, &label) else {
                break;
            };

            // 只取沿边那一个轴：竖直边看 y，水平边看 x。
            // 这就是「刘海永远出不了这条边」的实现方式。
            let vertical = is_vertical(&start.edge);
            let delta = if vertical {
                cur.y - start.cursor.1
            } else {
                cur.x - start.cursor.0
            };
            let span = if vertical { mon.h } else { mon.w } as f64;
            if span > 0.0 {
                let ratio = (start.ratio + delta / span).clamp(0.0, 1.0);
                if (ratio - along(&start.edge)).abs() > 1e-4 {
                    set_along(&start.edge, ratio);
                    place_notch(&app);
                }
            }
            std::thread::sleep(Duration::from_millis(DRAG_TICK_MS));
        }
        set_dragging(&label, false);
        // 收尾再摆一次，让落点正好压在边缘上
        eprintln!("ark-bar notch slid along {edge} to {:.3}", along(&edge));
        save_state(&app);
        // 沿边比例是整队共享的：所有刘海一起挪到同一个比例
        place_notch(&app);
        let _ = app.emit_to(&label, "drag_end", ());
    });
}

#[tauri::command]
pub fn end_notch_drag(window: WebviewWindow) {
    let label = window.label().to_string();
    set_dragging(&label, false);
    place_notch(window.app_handle());
}

/// 前端的「保持展开」状态（右键菜单要勾选它）：设置面板切模式时同步过来。
static NOTCH_MODE: Mutex<String> = Mutex::new(String::new());

#[tauri::command]
pub fn set_notch_mode(mode: String) {
    if let Ok(mut g) = NOTCH_MODE.lock() {
        *g = mode;
    }
}

/// 刘海右键菜单（Codenotch 的 Refresh now / keep open / Quit）。
/// Tauri 把菜单事件发给所有处理器，托盘那份也在内，所以这些 id 自带前缀各自过滤。
const MENU_PREFIX: &str = "notch:";

pub fn setup_menu(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("float") {
        w.on_menu_event(|w, ev| {
            let app = w.app_handle();
            match ev.id().as_ref() {
                "notch:refresh" => {
                    let _ = app.emit_to("float", "notch_refresh", ());
                }
                "notch:keep_open" => {
                    let _ = app.emit_to("float", "notch_keep_open", ());
                }
                "notch:settings" => crate::tray::show_settings_window(app),
                "notch:quit" => app.exit(0),
                _ => {}
            }
        });
    }
}

#[tauri::command]
pub fn show_notch_menu(app: AppHandle, _provider: Option<String>) -> Result<(), String> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
    let err = |e: tauri::Error| e.to_string();
    let keep_open_now = NOTCH_MODE
        .lock()
        .map(|m| m.as_str() == "always")
        .unwrap_or(false);

    let refresh = MenuItemBuilder::with_id(format!("{MENU_PREFIX}refresh"), "立即刷新")
        .build(&app)
        .map_err(err)?;
    let keep_open = CheckMenuItemBuilder::with_id(format!("{MENU_PREFIX}keep_open"), "保持展开")
        .checked(keep_open_now)
        .build(&app)
        .map_err(err)?;
    let settings = MenuItemBuilder::with_id(format!("{MENU_PREFIX}settings"), "偏好设置…")
        .build(&app)
        .map_err(err)?;
    let quit = MenuItemBuilder::with_id(format!("{MENU_PREFIX}quit"), "退出 ArkBar")
        .build(&app)
        .map_err(err)?;
    let menu = MenuBuilder::new(&app)
        .item(&refresh)
        .separator()
        .item(&keep_open)
        .separator()
        .item(&settings)
        .separator()
        .item(&quit)
        .build()
        .map_err(err)?;

    let Some(w) = app.get_webview_window("float") else {
        return Ok(());
    };
    // 返回时菜单已经关闭
    w.popup_menu(&menu).map_err(err)?;
    Ok(())
}

/// 屏幕配置（分辨率、缩放、插拔）变了就把刘海摆回去；拖动中不插手。
/// 上游在 Windows 上盯的是工作区（任务栏会移动），macOS 对齐物理边缘，所以这里盯整屏。
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let snapshot = |app: &AppHandle| -> Vec<(i32, i32, i32, i32, i64, Option<String>)> {
            desired_screens(app)
                .into_iter()
                .map(|s| (s.x, s.y, s.w, s.h, s.scale as i64, s.name.clone()))
                .collect()
        };
        let mut last = snapshot(&app);
        loop {
            std::thread::sleep(Duration::from_millis(WATCH_MS));
            if any_dragging() {
                continue;
            }
            let now = snapshot(&app);
            if now == last {
                continue;
            }
            // 显示器插拔/改分辨率：补齐或收掉窗口，再整队摆一遍
            eprintln!("ark-bar screens changed: {} -> {} screen(s)", last.len(), now.len());
            last = now;
            reconcile_fleet(&app);
            place_notch(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        // 2560x1440 的屏，左上原点
        Screen {
            name: Some("test".into()),
            x: 0,
            y: 0,
            w: 2560,
            h: 1440,
            scale: 2.0,
        }
    }

    #[test]
    fn right_edge_is_flush_and_centred() {
        let s = screen();
        let (x, y) = edge_origin(&s, "right", 880, 1360, 0.5);
        assert_eq!(x, 2560 - 880); // 右边缘严丝合缝
        assert_eq!(y, 720 - 680); // 垂直居中
    }

    #[test]
    fn top_edge_is_flush_and_centred() {
        let s = screen();
        let (x, y) = edge_origin(&s, "top", 1400, 960, 0.5);
        assert_eq!(y, 0);
        assert_eq!(x, 1280 - 700);
    }

    #[test]
    fn ratio_is_clamped_inside_the_edge() {
        let s = screen();
        // 比例被拖到 2.0（远超 1.0）也不会跑出屏幕
        let (x, _) = edge_origin(&s, "top", 1400, 960, 2.0);
        assert_eq!(x, 2560 - 1400);
        let (x, _) = edge_origin(&s, "top", 1400, 960, -1.0);
        assert_eq!(x, 0);
    }

    fn scr(x: i32, y: i32, w: i32, h: i32) -> Screen {
        Screen { name: Some(format!("{x},{y}")), x, y, w, h, scale: 2.0 }
    }

    #[test]
    fn main_scope_keeps_only_the_primary() {
        let all = [scr(0, 0, 5120, 2880), scr(5120, 0, 3840, 2160)];
        let plan = plan_screens(&all, Some(&all[0]), "main");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].x, 0);
    }

    #[test]
    fn all_scope_puts_the_primary_first_and_the_rest_in_order() {
        let all = [scr(5120, 0, 3840, 2160), scr(-1920, 0, 1920, 1080), scr(0, 0, 5120, 2880)];
        let plan = plan_screens(&all, Some(&all[2]), "all");
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].x, 0, "主屏第一");
        assert_eq!(plan[1].x, -1920, "其余按位置排序");
        assert_eq!(plan[2].x, 5120);
    }

    #[test]
    fn labels_and_screens_line_up() {
        assert_eq!(notch_label(0), "float");
        assert_eq!(notch_label(1), "float-1");
        assert_eq!(label_index("float"), 0);
        assert_eq!(label_index("float-3"), 3);
    }

    #[test]
    fn a_window_taller_than_the_screen_pins_to_the_top() {
        let s = screen();
        let (_, y) = edge_origin(&s, "right", 880, 2000, 0.5);
        assert_eq!(y, 0); // 夹取范围塌缩时取最小值，而不是 panic
    }
}
