use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_positioner::{Position, WindowExt};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../icons/tray-icon@2x.png");

    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../icons/32x32.png");

    let tray_image = Image::from_bytes(icon_bytes)?;

    let show_i = MenuItem::with_id(app, "show", "打开 ArkBar", true, None::<&str>)?;
    let float_i = MenuItem::with_id(app, "toggle_float", "切换桌面悬浮窗", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出 ArkBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &float_i, &quit_i])?;

    let mut builder = TrayIconBuilder::with_id("ark-bar-tray")
        .tooltip("ArkBar - 多模型配额监控")
        .icon(tray_image)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.move_window_constrained(Position::TrayCenter);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "toggle_float" => {
                    if let Some(window) = app.get_webview_window("float") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.move_window(Position::TopRight);
                            let _ = window.show();
                            let _ = window.set_focus();
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
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.move_window_constrained(Position::TrayCenter);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
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
    if let Some(tray) = app.tray_by_id("ark-bar-tray") {
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(Some(&title));

        #[cfg(not(target_os = "macos"))]
        {
            let trimmed = title.trim();
            let tip = if trimmed.is_empty() {
                "ArkBar - 火山方舟配额监控".to_string()
            } else {
                format!("ArkBar - 火山方舟配额监控 [{}]", trimmed)
            };
            let _ = tray.set_tooltip(Some(&tip));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.move_window_constrained(Position::TrayCenter);
        let _ = window.show();
        let _ = window.set_focus();
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
        let _ = window.move_window(Position::TopRight);
        let _ = window.show();
        let _ = window.set_focus();
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

