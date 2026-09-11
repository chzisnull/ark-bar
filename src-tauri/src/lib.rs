mod env_resolver;
mod ark_cli;
mod tray;
mod updater;
mod provider_models;
mod provider_antigravity;
mod provider_grok;
mod provider_codex;
mod token_store;
mod provider_manager;

use tauri::Manager;
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize PATH environment for macOS GUI applications
    env_resolver::init_effective_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            // Set macOS activation policy to Accessory so it doesn't show in Dock
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Initialize Menu Bar Tray
            tray::setup_tray(app.handle())?;

            // Hide window when clicking outside (loss of focus)
            if let Some(window) = app.get_webview_window("main") {
                let win_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = win_clone.hide();
                    }
                });

                // Auto popup on launch so user immediately knows ArkBar is running!
                #[cfg(target_os = "macos")]
                {
                    if window.move_window_constrained(Position::TrayCenter).is_err() {
                        let _ = window.move_window(Position::TopRight);
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if window.move_window_constrained(Position::TrayCenter).is_err() {
                        let _ = window.move_window(Position::BottomRight);
                    }
                }

                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ark_cli::check_environment,
            ark_cli::install_arkcli,
            ark_cli::login_volc_sso,
            ark_cli::get_sso_auth_url,
            ark_cli::login_with_code,
            ark_cli::get_usage_plan,
            ark_cli::check_for_updates,
            updater::install_app_update,
            provider_manager::get_unified_usage,
            provider_manager::get_all_providers_usage,
            provider_manager::set_provider_token,
            provider_manager::read_provider_token,
            tray::update_tray_title,
            tray::hide_window,
            tray::open_float_window,
            tray::close_float_window,
            tray::is_float_window_open,
            tray::start_drag,
            tray::drag_move_window,
        ])
        .run(tauri::generate_context!())
        .expect("运行 ArkBar 应用时出错");
}
