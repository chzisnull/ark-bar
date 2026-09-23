mod env_resolver;
mod ark_cli;
mod tray;
mod updater;
mod background;
mod provider_models;
mod provider_antigravity;
mod provider_grok;
mod provider_codex;
mod provider_teamo;
mod token_store;
mod provider_manager;
mod usage_cache;
pub mod token_stats;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize PATH environment for macOS GUI applications
    env_resolver::init_effective_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // 菜单栏图标被系统挤掉或被用户隐藏时，快捷键是常驻入口；
                    // 行为与托盘左键一致：显则隐、隐则显
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("float") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                tray::reveal_float_window(&window);
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Set macOS activation policy to Accessory so it doesn't show in Dock
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Initialize Menu Bar Tray
            tray::setup_tray(app.handle())?;

            // Disable macOS App Nap: with every window hidden the system may
            // suspend the whole process, which would freeze the background
            // sync thread and stall the tray percentage. The activity handle
            // is intentionally kept for the process lifetime.
            #[cfg(target_os = "macos")]
            {
                use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
                let info = NSProcessInfo::processInfo();
                // NSActivityUserInitiatedAllowingIdleSystemSleep
                // = 0x00FFFFFF & !NSActivityIdleSystemSleepDisabled(1 << 20)
                let activity = info.beginActivityWithOptions_reason(
                    NSActivityOptions(0x00EF_FFFF),
                    &NSString::from_str("ArkBar 定时同步各平台配额"),
                );
                std::mem::forget(activity);
            }

            // Native periodic usage sync (webview timers are suspended while
            // the popover window is hidden).
            background::spawn(app.handle().clone());

            // 全局快捷键 ⌘/Ctrl+Shift+A 呼出主面板。注册失败（如与其他应用
            // 冲突）只降级不崩溃：悬浮窗右键菜单仍是兜底入口。
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Err(e) = app.global_shortcut().register("CommandOrControl+Shift+A") {
                    eprintln!("注册全局快捷键失败: {e}");
                }
            }

            // Hide window when clicking outside (loss of focus).
            // Ignore blur for a short window after tray-clicks so the popover
            // does not hide itself while macOS is still delivering the click.
            // Automatically reveal the right-side screen Notch on launch!
            if let Some(float_window) = app.get_webview_window("float") {
                tray::reveal_float_window(&float_window);
            }

            // Hide menu bar icon by default per user request ("顶部菜单栏都可以不用展示")
            if let Some(tray) = app.tray_by_id("ark-bar-tray") {
                let _ = tray.set_visible(false);
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
            background::set_background_interval,
            background::set_tray_prefs,
            provider_manager::get_unified_usage,
            provider_manager::peek_cached_usage,
            provider_manager::get_all_providers_usage,
            provider_manager::set_provider_token,
            provider_manager::read_provider_token,
            tray::update_tray_title,
            tray::set_tray_icon_visible,
            tray::show_main_window,
            tray::hide_window,
            tray::open_float_window,
            tray::close_float_window,
            tray::is_float_window_open,
            tray::start_drag_move,
            tray::update_drag_move,
            tray::end_drag_move,
            tray::exit_app,
            tray::set_float_window_size,
            tray::set_main_window_size,
        ])
        .run(tauri::generate_context!())
        .expect("运行 ArkBar 应用时出错");
}
