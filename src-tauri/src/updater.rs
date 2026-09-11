use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UpdateProgress {
    pub stage: String, // "preparing", "downloading", "extracting", "installing", "restarting", "error", "done"
    pub percent: f64,
    pub current_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
}

/// Determine default asset filename based on current OS and CPU architecture
pub fn get_platform_asset_filename(version: &str) -> String {
    let clean_version = version.trim_start_matches('v');
    #[cfg(target_os = "macos")]
    {
        let _ = clean_version;
        if cfg!(target_arch = "aarch64") {
            "ArkBar_aarch64.app.tar.gz".to_string()
        } else {
            "ArkBar_x64.app.tar.gz".to_string()
        }
    }
    #[cfg(target_os = "windows")]
    {
        format!("ArkBar_{}_x64-setup.exe", clean_version)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        format!("ArkBar_{}_amd64.AppImage", clean_version)
    }
}

/// Helper to emit update progress to all frontend windows
fn emit_progress(
    app: &AppHandle,
    stage: &str,
    percent: f64,
    current_bytes: u64,
    total_bytes: u64,
    message: &str,
) {
    let _ = app.emit(
        "update-progress",
        UpdateProgress {
            stage: stage.to_string(),
            percent: (percent * 10.0).round() / 10.0,
            current_bytes,
            total_bytes,
            message: message.to_string(),
        },
    );
}

/// Query Content-Length using curl -sIL to know total download size
fn probe_content_length(url: &str) -> u64 {
    let output = crate::env_resolver::create_command("curl")
        .args(&["-sIL", "--connect-timeout", "10", "-H", "User-Agent: ark-bar-app", url])
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut last_len = 0u64;
        for line in text.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                let val_str = line["content-length:".len()..].trim();
                if let Ok(bytes) = val_str.parse::<u64>() {
                    if bytes > 0 {
                        last_len = bytes;
                    }
                }
            }
        }
        return last_len;
    }
    0
}

/// Finds the root `.app` bundle path if running inside a bundle on macOS
#[cfg(target_os = "macos")]
fn get_current_app_bundle_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut p = exe.as_path();
        while let Some(parent) = p.parent() {
            if parent.extension().and_then(|e| e.to_str()) == Some("app") {
                return parent.to_path_buf();
            }
            p = parent;
        }
    }
    PathBuf::from("/Applications/ArkBar.app")
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    version: String,
    custom_url: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_update_sync(&app, &version, custom_url)
    })
    .await
    .map_err(|e| format!("执行更新任务崩溃: {}", e))?
}

fn execute_update_sync(
    app: &AppHandle,
    version: &str,
    custom_url: Option<String>,
) -> Result<(), String> {
    let clean_version = version.trim_start_matches('v').to_string();
    let repo = "chzisnull/ark-bar";
    let filename = get_platform_asset_filename(&clean_version);

    let download_url = custom_url.unwrap_or_else(|| {
        format!(
            "https://github.com/{}/releases/download/v{}/{}",
            repo, clean_version, filename
        )
    });

    emit_progress(app, "preparing", 0.0, 0, 0, "正在连接下载节点...");

    let temp_dir = std::env::temp_dir().join(format!("arkbar_update_{}", clean_version));
    let _ = fs::create_dir_all(&temp_dir);
    let downloaded_file = temp_dir.join(&filename);
    let _ = fs::remove_file(&downloaded_file);

    // 1. Probe total size for progress calculation
    let total_bytes = probe_content_length(&download_url);
    emit_progress(app, "downloading", 0.0, 0, total_bytes, "正在下载更新包...");

    // 2. Spawn curl download child process
    let mut curl_cmd = crate::env_resolver::create_command("curl");
    curl_cmd.args(&[
        "-L",
        "-f",
        "-s",
        "-S",
        "--connect-timeout",
        "20",
        "-H",
        "User-Agent: ark-bar-app",
        "-o",
        downloaded_file.to_str().unwrap(),
        &download_url,
    ]);

    let mut child = curl_cmd.spawn().map_err(|e| format!("启动下载失败: {}", e))?;

    // Poll download progress while child process is running
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let err_msg = format!("下载更新包失败 (退出码 {:?})，请检查网络或稍后重试", status.code());
                    emit_progress(app, "error", 0.0, 0, total_bytes, &err_msg);
                    return Err(err_msg);
                }
                break;
            }
            Ok(None) => {
                let current_bytes = fs::metadata(&downloaded_file)
                    .map(|m| m.len())
                    .unwrap_or(0);

                let percent = if total_bytes > 0 {
                    ((current_bytes as f64 / total_bytes as f64) * 100.0).clamp(0.0, 99.0)
                } else {
                    0.0
                };

                let mb_curr = current_bytes as f64 / (1024.0 * 1024.0);
                let mb_total = total_bytes as f64 / (1024.0 * 1024.0);
                let msg = if total_bytes > 0 {
                    format!("正在下载更新包 ({:.1} MB / {:.1} MB)...", mb_curr, mb_total)
                } else {
                    format!("正在下载更新包 ({:.1} MB)...", mb_curr)
                };

                emit_progress(app, "downloading", percent, current_bytes, total_bytes, &msg);
                thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                let err_msg = format!("监控下载失败: {}", e);
                emit_progress(app, "error", 0.0, 0, total_bytes, &err_msg);
                return Err(err_msg);
            }
        }
    }

    emit_progress(
        app,
        "installing",
        100.0,
        total_bytes,
        total_bytes,
        "下载完成，正在准备安装更新...",
    );

    // 3. Platform-specific installation & restart
    #[cfg(target_os = "macos")]
    {
        install_and_restart_macos(app, &temp_dir, &downloaded_file)?;
    }

    #[cfg(target_os = "windows")]
    {
        install_and_restart_windows(app, &temp_dir, &downloaded_file)?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let msg = "当前系统不支持自动解压安装，请前往 GitHub 下载发布包".to_string();
        emit_progress(app, "error", 0.0, 0, 0, &msg);
        return Err(msg);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn install_and_restart_macos(
    app: &AppHandle,
    temp_dir: &Path,
    downloaded_archive: &Path,
) -> Result<(), String> {
    emit_progress(app, "extracting", 100.0, 0, 0, "正在解压新版应用...");

    let extract_dir = temp_dir.join("extracted");
    let _ = fs::remove_dir_all(&extract_dir);
    let _ = fs::create_dir_all(&extract_dir);

    // Extract tar.gz
    let tar_status = Command::new("tar")
        .args(&[
            "-xzf",
            downloaded_archive.to_str().unwrap(),
            "-C",
            extract_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("执行 tar 解压失败: {}", e))?;

    if !tar_status.success() {
        let err_msg = "解压安装包失败，可能文件损坏".to_string();
        emit_progress(app, "error", 0.0, 0, 0, &err_msg);
        return Err(err_msg);
    }

    // Locate ArkBar.app inside extract_dir
    let mut extracted_app = extract_dir.join("ArkBar.app");
    if !extracted_app.exists() {
        if let Ok(entries) = fs::read_dir(&extract_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("app") {
                    extracted_app = entry.path();
                    break;
                }
            }
        }
    }

    if !extracted_app.exists() {
        let err_msg = "解压后未找到 ArkBar.app 应用程序".to_string();
        emit_progress(app, "error", 0.0, 0, 0, &err_msg);
        return Err(err_msg);
    }

    // Remove quarantine on the extracted app bundle beforehand
    let _ = Command::new("xattr")
        .args(&["-dr", "com.apple.quarantine", extracted_app.to_str().unwrap()])
        .status();

    let target_app = get_current_app_bundle_path();
    let target_pid = std::process::id();

    // Create a robust detached restart script
    let script_path = temp_dir.join("restart_arkbar.sh");
    let script_content = format!(
        r#"#!/bin/bash
TARGET_PID="{}"
EXTRACTED_APP="{}"
TARGET_APP="{}"
CLEANUP_DIR="{}"

# Wait for current app process to exit
for i in {{1..30}}; do
    if ! kill -0 "$TARGET_PID" 2>/dev/null; then
        break
    fi
    sleep 0.3
done

# If still running, force terminate
kill -9 "$TARGET_PID" 2>/dev/null || true

# Replace old app with new app
rm -rf "$TARGET_APP"
cp -R "$EXTRACTED_APP" "$TARGET_APP"
xattr -dr com.apple.quarantine "$TARGET_APP" 2>/dev/null || true

# Launch the updated application
open "$TARGET_APP"

# Clean up temporary update directory
rm -rf "$CLEANUP_DIR"
rm -f "$0"
"#,
        target_pid,
        extracted_app.to_string_lossy(),
        target_app.to_string_lossy(),
        temp_dir.to_string_lossy()
    );

    fs::write(&script_path, script_content)
        .map_err(|e| format!("写入安装重启脚本失败: {}", e))?;

    // Make script executable
    let _ = Command::new("chmod")
        .args(&["+x", script_path.to_str().unwrap()])
        .status();

    emit_progress(
        app,
        "restarting",
        100.0,
        0,
        0,
        "更新安装就绪，正在自动重启应用...",
    );

    // Spawn detached process
    let _ = Command::new("nohup")
        .args(&["sh", script_path.to_str().unwrap()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    thread::sleep(Duration::from_millis(600));
    app.exit(0);
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_and_restart_windows(
    app: &AppHandle,
    temp_dir: &Path,
    installer_exe: &Path,
) -> Result<(), String> {
    emit_progress(app, "installing", 100.0, 0, 0, "正在启动静默更新安装程序...");

    let target_pid = std::process::id();
    let bat_path = temp_dir.join("update_arkbar.bat");

    // Write Windows detached batch script
    let bat_content = format!(
        r#"@echo off
set "TARGET_PID={}"
set "SETUP_EXE={}"

:wait_loop
tasklist /fi "PID eq %TARGET_PID%" 2>nul | find "%TARGET_PID%" >nul
if %ERRORLEVEL% equ 0 (
    timeout /t 1 /nobreak >nul
    goto wait_loop
)

:: Run NSIS installer silently
start /wait "" "%SETUP_EXE%" /S

:: Launch newly updated ArkBar
if exist "%LOCALAPPDATA%\Programs\ArkBar\ArkBar.exe" (
    start "" "%LOCALAPPDATA%\Programs\ArkBar\ArkBar.exe"
) else (
    if exist "%ProgramFiles%\ArkBar\ArkBar.exe" (
        start "" "%ProgramFiles%\ArkBar\ArkBar.exe"
    )
)

:: Clean up installer and batch file
del "%SETUP_EXE%" >nul 2>nul
(goto) 2>nul & del "%~f0"
"#,
        target_pid,
        installer_exe.to_string_lossy()
    );

    fs::write(&bat_path, bat_content)
        .map_err(|e| format!("写入 Windows 更新脚本失败: {}", e))?;

    emit_progress(
        app,
        "restarting",
        100.0,
        0,
        0,
        "更新安装就绪，正在自动重启应用...",
    );

    // Spawn completely silent detached batch process
    let mut cmd = crate::env_resolver::create_command("cmd.exe");
    cmd.args(&[
        "/C",
        "start",
        "",
        "/B",
        bat_path.to_str().unwrap(),
    ]);

    let _ = cmd.spawn();

    thread::sleep(Duration::from_millis(600));
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_platform_asset_filename() {
        let filename = get_platform_asset_filename("v0.1.2");
        assert!(!filename.is_empty());
        #[cfg(target_os = "macos")]
        {
            assert!(filename.ends_with(".app.tar.gz"));
        }
        #[cfg(target_os = "windows")]
        {
            assert!(filename.ends_with("_x64-setup.exe"));
        }
    }
}
