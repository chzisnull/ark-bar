//! 落盘日志 —— 上游同样有一份（`applog`）。
//!
//! 刘海「看不见」「不在这块屏上」「点了没反应」这类问题，只有当事人机器上的现场
//! 才能解释；stdout 在 macOS 上从 Finder 启动时根本没有去处。所以关键路径
//! （舰队对账、摆放、点穿、活动状态变化）都往这个文件里记一行，
//! 出问题时让用户把 `<app data dir>/arkbar.log` 发回来即可。

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 超过这个大小就只保留尾部，避免常年运行把磁盘写满
const MAX_BYTES: u64 = 512 * 1024;
static LOCK: Mutex<()> = Mutex::new(());
/// 持久写入句柄：避免每条日志都 open/close，写入经 BufWriter 批量落盘
static WRITER: OnceLock<Mutex<Option<BufWriter<std::fs::File>>>> = OnceLock::new();

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

fn open_writer(path: &std::path::Path) -> Option<BufWriter<std::fs::File>> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
        .map(BufWriter::new)
}

/// 流式轮转：只 Seek 读尾部 `MAX_BYTES/2` 再整写回，避免把整个 512KB 读进内存。
fn rotate(path: &std::path::Path) {
    use std::io::{Read, Seek, SeekFrom, Write};
    let Ok(mut src) = std::fs::File::open(path) else {
        return;
    };
    let Ok(len) = src.metadata().map(|m| m.len()) else {
        return;
    };
    if src
        .seek(SeekFrom::Start(len.saturating_sub(MAX_BYTES / 2)))
        .is_err()
    {
        return;
    }
    let mut buf = Vec::new();
    if src.read_to_end(&mut buf).is_err() {
        return;
    }
    drop(src);
    if let Ok(mut dst) = std::fs::OpenOptions::new().write(true).truncate(true).open(path) {
        let _ = dst.write_all(&buf);
    }
}

pub fn log(msg: &str) {
    // 顺带打一份到 stderr：开发时直接看终端
    eprintln!("{msg}");

    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let _guard = LOCK.lock();
    let slot = WRITER.get_or_init(|| Mutex::new(None));
    let Ok(mut writer) = slot.lock() else {
        return;
    };
    if writer.is_none() {
        *writer = open_writer(path);
    }
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_BYTES {
            if let Some(mut w) = writer.take() {
                let _ = w.flush();
            }
            rotate(path);
            *writer = open_writer(path);
        }
    }
    if let Some(w) = writer.as_mut() {
        let _ = writeln!(w, "[{}] {}", stamp(), msg);
        // 日志是排障第一现场，不能因为缓冲而在退出时丢掉最后几行
        let _ = w.flush();
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
