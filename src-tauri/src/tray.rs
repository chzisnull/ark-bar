use tauri::{
    image::Image,
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

    let mut builder = TrayIconBuilder::with_id("ark-bar-tray")
        .tooltip("ArkBar - 火山方舟配额监控")
        .icon(tray_image);

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

#[tauri::command]
pub fn start_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn drag_move_window(window: tauri::WebviewWindow, dx: f64, dy: f64) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let current_pos = window.outer_position().map_err(|e| e.to_string())?;
    let phys_dx = (dx * scale).round() as i32;
    let phys_dy = (dy * scale).round() as i32;
    let new_pos = tauri::PhysicalPosition::new(current_pos.x + phys_dx, current_pos.y + phys_dy);
    window.set_position(new_pos).map_err(|e| e.to_string())?;
    Ok(())
}
