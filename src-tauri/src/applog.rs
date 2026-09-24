//! 落盘日志 —— 上游同样有一份（`applog`）。
//!
//! 刘海「看不见」「不在这块屏上」「点了没反应」这类问题，只有当事人机器上的现场
//! 才能解释；stdout 在 macOS 上从 Finder 启动时根本没有去处。所以关键路径
//! （舰队对账、摆放、点穿、活动状态变化）都往这个文件里记一行，
//! 出问题时让用户把 `<app data dir>/arkbar.log` 发回来即可。

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 超过这个大小就只保留尾部，避免常年运行把磁盘写满
const MAX_BYTES: u64 = 512 * 1024;
static LOCK: Mutex<()> = Mutex::new(());

/// 由 lib.rs 在 setup 里调用一次（拿得到 app 数据目录之后）。
pub fn init(path: PathBuf) {
    let _ = LOG_PATH.set(path);
}

fn stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 只到秒的本地时间够用了：日志是给人看顺序的
    let dt = chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|d| d.with_timezone(&chrono::Local))
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| secs.to_string());
    dt
}

pub fn log(msg: &str) {
    // 顺带打一份到 stderr：开发时直接看终端
    eprintln!("{msg}");

    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let _guard = LOCK.lock();
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_BYTES {
            if let Ok(text) = std::fs::read_to_string(path) {
                let tail: String = text
                    .chars()
                    .rev()
                    .take((MAX_BYTES / 2) as usize)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let _ = std::fs::write(path, tail);
            }
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] {}", stamp(), msg);
    }
}

/// 日志文件位置，供界面/支持说明引用。
pub fn path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

/// 设置页「通用」里显示用：日志文件在哪。
#[tauri::command]
pub fn log_path() -> Option<String> {
    path().map(|p| p.to_string_lossy().to_string())
}
