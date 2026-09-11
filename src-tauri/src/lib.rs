mod env_resolver;
mod ark_cli;
mod tray;

use tauri::Manager;

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
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ark_cli::check_environment,
            ark_cli::install_arkcli,
            ark_cli::login_volc_sso,
            ark_cli::get_usage_plan,
            ark_cli::check_for_updates,
            tray::update_tray_title,
            tray::hide_window,
            tray::open_float_window,
            tray::close_float_window,
            tray::is_float_window_open,
        ])
        .run(tauri::generate_context!())
        .expect("运行 ArkBar 应用时出错");
}
