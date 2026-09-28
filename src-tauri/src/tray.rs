use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{
    image::Image,
    Emitter,
    menu::{ContextMenu, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewWindow,
};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_positioner::{Position, WindowExt};

use crate::notch;

/// 点击菜单栏图标才弹出用量（而不是悬停顶部/右侧刘海）。
/// 前端 localStorage 是持久化来源，启动时由前端写回这里。
static TRAY_CLICK_USAGE: AtomicBool = AtomicBool::new(false);

pub fn tray_click_usage_enabled() -> bool {
    TRAY_CLICK_USAGE.load(Ordering::Relaxed)
}

static LAST_TRAY_TITLE: Mutex<String> = Mutex::new(String::new());

pub fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 显示全部刘海（每块屏一个）。位置由 notch::place_notch 决定：贴着当前边、
/// 按记住的沿边比例摆放，不再「只摆第一次」。
pub fn reveal_float_window(window: &WebviewWindow) {
    let app = window.app_handle();
    notch::reconcile_fleet(app);
    notch::place_notch(app);
    for label in notch::notch_labels(app) {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.show();
        }
    }
}

/// 所有刘海是否都藏着（托盘/快捷键判断用：只要有一个可见就算可见）。
fn any_notch_visible(app: &AppHandle) -> bool {
    notch::notch_labels(app).iter().any(|l| {
        app.get_webview_window(l)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    })
}

fn hide_all_notches(app: &AppHandle) {
    for label in notch::notch_labels(app) {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.hide();
        }
    }
}

fn usage_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("usage")
}

fn usage_popover_visible(app: &AppHandle) -> bool {
    usage_window(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 失焦自动收起的时间戳。托盘再次点击会先让弹出窗失焦、再派发点击事件，
/// 不记这一笔的话「点图标收起」会变成「收起后又立刻弹出」。
static LAST_BLUR_HIDE: Mutex<Option<std::time::Instant>> = Mutex::new(None);

const BLUR_HIDE_GUARD_MS: u128 = 350;

fn note_blur_hide() {
    if let Ok(mut g) = LAST_BLUR_HIDE.lock() {
        *g = Some(std::time::Instant::now());
    }
}

fn blurred_just_now() -> bool {
    LAST_BLUR_HIDE
        .lock()
        .ok()
        .and_then(|g| g.map(|t| t.elapsed().as_millis() < BLUR_HIDE_GUARD_MS))
        .unwrap_or(false)
}

fn hide_usage_popover(app: &AppHandle) {
    if let Some(w) = usage_window(app) {
        let _ = w.hide();
    }
}

/// 用量卡片失焦即收起（点桌面别处、点其他应用都算）。
pub fn attach_usage_window(app: &AppHandle) {
    let Some(usage) = usage_window(app) else {
        // 「点了菜单栏没反应」时第一个该看的就是这一行
        crate::applog::log("usage 窗口缺失：点击弹出会没反应（检查 tauri.conf.json 的窗口定义）");
        return;
    };
    crate::applog::log("usage 窗口就绪（点击菜单栏图标弹出）");
    let handle = app.clone();
    usage.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            note_blur_hide();
            if let Some(w) = handle.get_webview_window("usage") {
                let _ = w.hide();
            }
        }
    });
}

/// 最近一次托盘图标的矩形（物理像素、左上原点）。
///
/// 位置不能直接用 tauri-plugin-positioner 的 TrayBottomCenter：它把窗口顶边
/// 放在图标顶边上，而菜单栏里的图标顶边 y 实测约 -2 —— 窗口贴到屏幕最顶端，
/// 上半截被菜单栏盖住。这里只借它的 x（图标水平位置是准的），竖直方向改用
/// 工作区顶边（macOS 的 visibleFrame 顶 = 菜单栏底边，实测 60px）。
static TRAY_RECT: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

/// 菜单栏底边再往下留的缝，别让卡片贴着菜单栏。
const MENU_BAR_GAP: i32 = 6;

/// 纯计算：用量卡片左上角（物理像素）。
///
/// `work` = 工作区 (left, top, right, bottom)，macOS 上 top 就是菜单栏底边；
/// `tray` = 图标矩形 (x, y, w, h)。水平居中于图标、竖直贴菜单栏下方，
/// 两者都夹进工作区，保证窄屏/图标贴边时整块可见。
fn popover_position(
    work: (i32, i32, i32, i32),
    tray: Option<(f64, f64, f64, f64)>,
    window: (i32, i32),
) -> (i32, i32) {
    let (left, top, right, bottom) = work;
    let (ww, wh) = window;

    let want_x = match tray {
        Some((x, _, w, _)) => x as i32 + w as i32 / 2 - ww / 2,
        None => left + (right - left - ww) / 2,
    };
    let max_x = (right - ww).max(left);
    let x = want_x.clamp(left, max_x);

    // 下方放不下就往上收，但永不越过菜单栏底边
    let max_y = (bottom - wh).max(top);
    let y = (top + MENU_BAR_GAP).min(max_y);
    (x, y)
}

/// 菜单栏下方的第一条可用像素线：用量卡片贴在这里才不会被菜单栏吃掉。
fn popover_work_area(window: &WebviewWindow, tray: Option<(f64, f64, f64, f64)>) -> Option<(i32, i32, i32, i32)> {
    let monitor = tray
        .and_then(|(x, y, _, _)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())?;
    let work = monitor.work_area();
    let left = work.position.x;
    let top = work.position.y;
    let right = left + work.size.width as i32;
    let bottom = top + work.size.height as i32;
    Some((left, top, right, bottom))
}

/// 把用量卡片摆到菜单栏下方、水平对齐刚点过的图标。
fn place_usage_popover(window: &WebviewWindow) {
    let tray = TRAY_RECT.lock().ok().and_then(|g| *g);
    let Ok(size) = window.outer_size() else {
        return;
    };
    let (ww, wh) = (size.width as i32, size.height as i32);

    #[cfg(target_os = "macos")]
    {
        let Some(work) = popover_work_area(window, tray) else {
            return;
        };
        let (x, y) = popover_position(work, tray, (ww, wh));
        crate::applog::log(&format!(
            "usage popover placed at ({x},{y}) work={work:?} tray={tray:?} win=({ww},{wh})"
        ));
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows 的任务栏在底部，图标上方才是正确方向
        let _ = (ww, wh, tray);
        let _ = window.move_window_constrained(Position::TrayCenter);
    }
}

/// 把用量卡片贴到刚点过的菜单栏图标下方（或上方），再显示。
fn show_usage_popover(app: &AppHandle) {
    let Some(window) = usage_window(app) else {
        return;
    };
    place_usage_popover(&window);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to("usage", "usage-popover-shown", ());
}

fn remember_tray_rect(x: f64, y: f64, w: f64, h: f64) {
    if let Ok(mut g) = TRAY_RECT.lock() {
        *g = Some((x, y, w, h));
    }
}

/// 点击图标时该对用量卡片做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopoverAction {
    /// 当前可见 → 收起
    Hide,
    /// 当前不可见、且不是刚被自己收起 → 弹出
    Show,
    /// 这一下点击就是刚才那次收起动作的余波，什么都别做
    Noop,
}

/// 纯判定：把「点图标收起」与「失焦收起」两条路径区分开。
/// 托盘点击到达时卡片往往已经因失焦藏好了，此时若照常弹回，
/// 就会变成「点一下收起又立刻弹出」。
fn popover_action(visible: bool, blurred_just_now: bool) -> PopoverAction {
    if visible {
        PopoverAction::Hide
    } else if blurred_just_now {
        PopoverAction::Noop
    } else {
        PopoverAction::Show
    }
}

fn toggle_usage_popover(app: &AppHandle) {
    match popover_action(usage_popover_visible(app), blurred_just_now()) {
        PopoverAction::Hide => hide_usage_popover(app),
        PopoverAction::Noop => {}
        PopoverAction::Show => {
            // 点击模式不走屏幕边缘刘海
            hide_all_notches(app);
            show_usage_popover(app);
        }
    }
}

pub fn open_usage_window(app: &AppHandle) {
    hide_all_notches(app);
    show_usage_popover(app);
}

pub fn is_usage_window_open(app: AppHandle) -> bool {
    usage_popover_visible(&app)
}

/// 左键：点击模式弹出用量；否则显隐屏幕刘海。
fn on_tray_left_click(app: &AppHandle) {
    if tray_click_usage_enabled() {
        toggle_usage_popover(app);
        return;
    }
    hide_usage_popover(app);
    if any_notch_visible(app) {
        hide_all_notches(app);
    } else if let Some(window) = app.get_webview_window("float") {
        reveal_float_window(&window);
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../icons/tray-icon@2x.png");

    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../icons/32x32.png");

    let tray_image = Image::from_bytes(icon_bytes)?;

    let show_i = MenuItem::with_id(app, "show", "偏好设置...", true, None::<&str>)?;
    let float_i = MenuItem::with_id(app, "toggle_float", "切换屏幕刘海 (Notch)", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出 ArkBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &float_i, &quit_i])?;
    // 弹出用副本：菜单不再常驻挂载到状态栏项（见下），由事件闭包持有
    let popup_menu = menu.clone();

    let mut builder = TrayIconBuilder::with_id("ark-bar-tray")
        .tooltip("ArkBar - 屏幕刘海配额监控")
        .icon(tray_image)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    show_settings_window(app);
                }
                "toggle_float" => {
                    on_tray_left_click(app);
                }
                _ => {}
            }
        });

    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }

    let _tray = builder
        .on_tray_icon_event(move |tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = &event
            {
                // 记下图标矩形：点击模式要靠它把用量卡片对齐到图标
                // （tray-icon 给的是 Logical/Physical 枚举，统一转成物理像素）
                let pos = rect.position.to_physical::<i32>(1.0);
                let size = rect.size.to_physical::<i32>(1.0);
                remember_tray_rect(
                    pos.x as f64,
                    pos.y as f64,
                    size.width as f64,
                    size.height as f64,
                );
                match (button, button_state) {
                    // 左键：点击模式弹出用量；否则显隐屏幕刘海
                    (MouseButton::Left, MouseButtonState::Up) => {
                        on_tray_left_click(tray.app_handle());
                    }
                    // 右键：在当前光标处手动弹出菜单（菜单未挂载到状态栏项）。
                    // 不传坐标时 muda 直接用 NSEvent mouseLocation 屏幕坐标，
                    // 与主窗口是否隐藏无关
                    (MouseButton::Right, MouseButtonState::Up) => {
                        let app = tray.app_handle();
                        if let Some(ww) = app.get_webview_window("main") {
                            if let Err(e) = popup_menu.popup(ww.as_ref().window()) {
                                eprintln!("弹出托盘菜单失败: {e}");
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[tauri::command]
pub fn exit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn update_tray_title(app: AppHandle, title: String) -> Result<(), String> {
    if let Ok(mut last) = LAST_TRAY_TITLE.lock() {
        if *last == title {
            return Ok(());
        }
        *last = title.clone();
    }

    if let Some(tray) = app.tray_by_id("ark-bar-tray") {
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(Some(&title));

        #[cfg(not(target_os = "macos"))]
        {
            let trimmed = title.trim();
            let tip = if trimmed.is_empty() {
                "ArkBar - 多模型配额监控".to_string()
            } else {
                format!("ArkBar - 多模型配额监控 [{}]", trimmed)
            };
            let _ = tray.set_tooltip(Some(&tip));
        }
    }
    Ok(())
}

/// 点击菜单栏图标才显示用量。开启后收起顶部/右侧刘海，左键改为弹出用量卡片。
#[tauri::command]
pub fn set_tray_click_usage(app: AppHandle, enabled: bool) -> Result<(), String> {
    TRAY_CLICK_USAGE.store(enabled, Ordering::Relaxed);
    if enabled {
        hide_all_notches(&app);
        hide_usage_popover(&app);
    } else {
        hide_usage_popover(&app);
        // 回到悬停刘海：把屏幕边缘刘海重新摆出来
        if let Some(window) = app.get_webview_window("float") {
            reveal_float_window(&window);
        }
    }
    // 刘海窗口藏在后台时未必收得到跨窗口的 localStorage 变更，
    // 这里直接广播一次，保证它的悬停开关跟当前模式一致。
    let _ = app.emit("tray_click_usage", enabled);
    Ok(())
}

#[tauri::command]
pub fn hide_usage_window(app: AppHandle) -> Result<(), String> {
    hide_usage_popover(&app);
    Ok(())
}

/// 显示/隐藏菜单栏图标。隐藏后应用仍常驻运行，通过悬浮窗的恢复按钮
/// 或全局快捷键找回（前端 localStorage 持久化，启动时由前端应用）。
#[tauri::command]
pub fn set_tray_icon_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("ark-bar-tray") {
        tray.set_visible(visible).map_err(|e| e.to_string())?;
        if visible {
            // set_visible(true) 会重建状态项（macOS 新 NSStatusItem /
            // Windows NIM_ADD），百分比标题会丢失；扰动去重键后重放上次
            // 标题，让恢复出来的图标立即带回百分比/告警。
            let last = LAST_TRAY_TITLE
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            if let Ok(mut l) = LAST_TRAY_TITLE.lock() {
                *l = "\u{0}".to_string();
            }
            let _ = update_tray_title(app.clone(), last);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app);
    Ok(())
}

/// 打开设置窗口并弹出更新弹窗：刘海提示「有新版本」时的按钮用它。
#[tauri::command]
pub fn open_update_modal(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app);
    let _ = app.emit_to("main", "open_update", ());
    Ok(())
}

/// 打开设置窗口并直接落到「安装 / 授权」引导：刘海卡片上未连接时的那个按钮用它。
#[tauri::command]
pub fn open_onboarding(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app);
    let _ = app.emit_to("main", "open_onboarding", ());
    Ok(())
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn open_float_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("float") {
        reveal_float_window(&window);
    }
    Ok(())
}

#[tauri::command]
pub fn close_float_window(app: AppHandle) -> Result<(), String> {
    hide_all_notches(&app);
    Ok(())
}

#[tauri::command]
pub fn is_float_window_open(app: AppHandle) -> bool {
    any_notch_visible(&app)
}

#[tauri::command]
pub fn set_float_window_size(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    // 尺寸由每块屏上的刘海自己按边决定（见 notch::notch_window_size）；
    // 这里只负责整队重新贴边，避免长出来的一截顶到屏幕外。
    let _ = (width, height);
    notch::place_notch(&app);
    Ok(())
}

#[tauri::command]
pub fn set_main_window_size(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_popover_closes_on_click() {
        assert_eq!(popover_action(true, false), PopoverAction::Hide);
    }

    #[test]
    fn hidden_popover_opens_on_click() {
        assert_eq!(popover_action(false, false), PopoverAction::Show);
    }

    #[test]
    fn blur_hide_racing_the_click_does_not_reopen() {
        // 点图标收起：失焦先发生，弹出窗已不可见且失焦就在刚才。
        // 这一下点击必须被吞掉，否则卡片会「收起后立刻弹回」。
        assert_eq!(popover_action(false, true), PopoverAction::Noop);
    }

    #[test]
    fn visible_popover_still_closes_even_right_after_a_blur() {
        // 失焦没能藏住（例如被别的路径重新显示）时，点击仍应能收起
        assert_eq!(popover_action(true, true), PopoverAction::Hide);
    }

    #[test]
    fn tray_click_usage_defaults_off() {
        // 默认仍是悬停刘海模式，老用户升级后行为不变
        assert!(!tray_click_usage_enabled());
    }

    // 下面这组用实测数字：本机 5120x2880 物理屏、菜单栏 30pt，
    // 工作区 = (0, 60, 5120, 2880)；图标矩形由 tray-icon 公式实测算出。
    const WORK: (i32, i32, i32, i32) = (0, 60, 5120, 2880);
    const TRAY: (f64, f64, f64, f64) = (3418.0, -2.0, 58.0, 66.0);
    const POPOVER: (i32, i32) = (800, 1240); // 400x620 逻辑 @2x

    #[test]
    fn popover_sits_below_the_menu_bar_and_centred_on_the_icon() {
        let (x, y) = popover_position(WORK, Some(TRAY), POPOVER);
        // 图标中心 3418+29=3447，减去半宽 400
        assert_eq!(x, 3047);
        // 菜单栏底边 60 + 6 的缝；不能是 -2（那会被菜单栏盖住）
        assert_eq!(y, 66);
    }

    #[test]
    fn popover_never_creeps_over_the_menu_bar() {
        // 图标顶边是负的（实测 -2），竖直方向绝不能直接采用它
        let (_, y) = popover_position(WORK, Some(TRAY), POPOVER);
        assert!(y >= WORK.1, "y={y} 越过了菜单栏底边 {}", WORK.1);
    }

    #[test]
    fn icon_at_the_far_right_is_clamped_into_view() {
        // 图标贴到屏幕最右时，居中会让卡片右半截出屏，必须夹回来
        let (x, _) = popover_position(WORK, Some((5100.0, -2.0, 58.0, 66.0)), POPOVER);
        assert_eq!(x, WORK.2 - POPOVER.0);
    }

    #[test]
    fn without_a_tray_rect_the_popover_is_centred() {
        let (x, y) = popover_position(WORK, None, POPOVER);
        assert_eq!(x, (WORK.2 - POPOVER.0) / 2);
        assert_eq!(y, WORK.1 + MENU_BAR_GAP);
    }

    #[test]
    fn a_popover_taller_than_the_work_area_still_starts_below_the_menu_bar() {
        let (_, y) = popover_position(WORK, Some(TRAY), (800, 3000));
        assert_eq!(y, WORK.1);
    }

    #[test]
    fn off_screen_tray_rect_is_clamped_to_the_left_edge() {
        let (x, _) = popover_position(WORK, Some((-500.0, -2.0, 58.0, 66.0)), POPOVER);
        assert_eq!(x, WORK.0);
    }
}



