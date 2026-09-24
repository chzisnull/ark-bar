use std::sync::Mutex;
use tauri::{
    image::Image,
    Emitter,
    menu::{ContextMenu, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewWindow,
};

use crate::notch;

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
                    if any_notch_visible(app) {
                        hide_all_notches(app);
                    } else if let Some(window) = app.get_webview_window("float") {
                        reveal_float_window(&window);
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
                    // 左键：显隐切换右侧屏幕刘海
                    (MouseButton::Left, MouseButtonState::Up) => {
                        let app = tray.app_handle();
                        if any_notch_visible(app) {
                            hide_all_notches(app);
                        } else if let Some(window) = app.get_webview_window("float") {
                            reveal_float_window(&window);
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
    show_settings_window(&app);
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



