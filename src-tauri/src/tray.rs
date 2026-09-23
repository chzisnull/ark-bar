use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{ContextMenu, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewWindow,
};
use tauri_plugin_positioner::{Position, WindowExt};

static FLOAT_PLACED: AtomicBool = AtomicBool::new(false);
pub static IGNORE_UNFOCUS_HIDE: AtomicBool = AtomicBool::new(false);
static LAST_TRAY_TITLE: Mutex<String> = Mutex::new(String::new());

fn arm_ignore_unfocus_hide() {
    IGNORE_UNFOCUS_HIDE.store(true, Ordering::SeqCst);
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(450));
        IGNORE_UNFOCUS_HIDE.store(false, Ordering::SeqCst);
    });
}

fn show_main_popover(window: &WebviewWindow) {
    arm_ignore_unfocus_hide();
    // 托盘从未被点击（如图标被隐藏后直接用快捷键呼出）时 positioner
    // 没有 TrayCenter 数据，降级到屏幕角落
    if window.move_window_constrained(Position::TrayCenter).is_err() {
        #[cfg(target_os = "macos")]
        let _ = window.move_window(Position::TopRight);
        #[cfg(not(target_os = "macos"))]
        let _ = window.move_window(Position::BottomRight);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

fn reveal_float_window(window: &WebviewWindow) {
    // Only snap to the default corner the first time. After the user drags
    // the widget, keep that position across hide/show and size changes.
    if !FLOAT_PLACED.swap(true, Ordering::SeqCst) {
        let _ = window.move_window(Position::RightCenter);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../icons/tray-icon@2x.png");

    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../icons/32x32.png");

    let tray_image = Image::from_bytes(icon_bytes)?;

    let show_i = MenuItem::with_id(app, "show", "打开 ArkBar", true, None::<&str>)?;
    let float_i = MenuItem::with_id(app, "toggle_float", "切换屏幕刘海 (Notch)", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出 ArkBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &float_i, &quit_i])?;
    // 弹出用副本：菜单不再常驻挂载到状态栏项（见下），由事件闭包持有
    let popup_menu = menu.clone();

    let mut builder = TrayIconBuilder::with_id("ark-bar-tray")
        .tooltip("ArkBar - 多模型配额监控")
        .icon(tray_image)
        // macOS 27 起，系统会接管挂载了菜单的状态栏项的左键（直接弹菜单，
        // 自定义点击处理收不到事件）。与上游 tray-icon 0.25.1 (#365) 的修复
        // 思路一致：菜单不常驻挂载，右键时经 popup_at 在光标处手动弹出。
        // 待 tauri 升级到含 tray-icon >= 0.25.1 的版本后可移除本变通。
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        show_main_popover(&window);
                    }
                }
                "toggle_float" => {
                    if let Some(window) = app.get_webview_window("float") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            reveal_float_window(&window);
                        }
                    }
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
                button, button_state, ..
            } = &event
            {
                match (button, button_state) {
                    // 左键：显隐切换主面板（与历史行为一致）
                    (MouseButton::Left, MouseButtonState::Up) => {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                show_main_popover(&window);
                            }
                        }
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
    if let Some(window) = app.get_webview_window("main") {
        show_main_popover(&window);
    }
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
    if let Some(window) = app.get_webview_window("float") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn is_float_window_open(app: AppHandle) -> bool {
    if let Some(window) = app.get_webview_window("float") {
        window.is_visible().unwrap_or(false)
    } else {
        false
    }
}

static DRAG_SESSION: std::sync::Mutex<Option<(i32, i32, f64)>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn start_drag_move(window: tauri::WebviewWindow) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut session = DRAG_SESSION.lock().map_err(|e| e.to_string())?;
    *session = Some((pos.x, pos.y, scale));
    Ok(())
}

#[tauri::command]
pub fn update_drag_move(window: tauri::WebviewWindow, total_dx: f64, total_dy: f64) -> Result<(), String> {
    let session = DRAG_SESSION.lock().map_err(|e| e.to_string())?;
    if let Some((start_x, start_y, scale)) = *session {
        let phys_dx = (total_dx * scale).round() as i32;
        let phys_dy = (total_dy * scale).round() as i32;
        let target_pos = tauri::PhysicalPosition::new(start_x + phys_dx, start_y + phys_dy);
        window.set_position(target_pos).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn end_drag_move() -> Result<(), String> {
    if let Ok(mut session) = DRAG_SESSION.lock() {
        *session = None;
    }
    Ok(())
}

#[tauri::command]
pub fn set_float_window_size(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("float") {
        let pos = window.outer_position().ok();
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
        if let Some(p) = pos {
            let _ = window.set_position(p);
        }
        FLOAT_PLACED.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn set_main_window_size(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
    Ok(())
}


