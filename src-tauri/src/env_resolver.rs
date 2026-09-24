use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

static PATH_INIT: Once = Once::new();
static ARKCLI_BIN: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

pub fn refresh_effective_path() {
    let current_path = env::var("PATH").unwrap_or_default();
    let mut new_paths: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // 1. NVM node versions first (user's active/latest node)
        if let Ok(home) = env::var("HOME") {
            let home_path = Path::new(&home);
            let nvm_dir = home_path.join(".nvm/versions/node");
            if nvm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                    let mut node_versions: Vec<PathBuf> = Vec::new();
                    for entry in entries.flatten() {
                        let bin_path = entry.path().join("bin");
                        if bin_path.exists() {
                            node_versions.push(bin_path);
                        }
                    }
                    node_versions.sort();
                    node_versions.reverse();
                    for nv in node_versions {
                        if !new_paths.contains(&nv) {
                            new_paths.push(nv);
                        }
                    }
                }
            }

            let extra_dirs = [
                home_path.join(".cargo/bin"),
                home_path.join(".local/bin"),
                home_path.join(".npm-global/bin"),
                home_path.join("Library/pnpm"),
                home_path.join(".yarn/bin"),
            ];
            for ed in extra_dirs {
                if ed.exists() && !new_paths.contains(&ed) {
                    new_paths.push(ed);
                }
            }
        }

        // 2. Standard Homebrew and macOS system paths
        let standard_candidates = [
            "/opt/homebrew/bin",
            "/opt/homebrew/sbin",
            "/usr/local/bin",
            "/usr/local/sbin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ];
        for p in standard_candidates {
            let path = PathBuf::from(p);
            if path.exists() && !new_paths.contains(&path) {
                new_paths.push(path);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Node and npm global paths
        if let Ok(appdata) = env::var("APPDATA") {
            let npm_path = PathBuf::from(&appdata).join("npm");
            if npm_path.exists() && !new_paths.contains(&npm_path) {
                new_paths.push(npm_path);
            }
        }
        if let Ok(localappdata) = env::var("LOCALAPPDATA") {
            let node_path = PathBuf::from(&localappdata).join("Programs").join("nodejs");
            if node_path.exists() && !new_paths.contains(&node_path) {
                new_paths.push(node_path);
            }
        }
        let win_candidates = [
            r"C:\Program Files\nodejs",
            r"C:\Program Files (x86)\nodejs",
        ];
        for p in win_candidates {
            let path = PathBuf::from(p);
            if path.exists() && !new_paths.contains(&path) {
                new_paths.push(path);
            }
        }
    }

    // Keep any existing paths
    for p in env::split_paths(&current_path) {
        if !new_paths.contains(&p) {
            new_paths.push(p);
        }
    }

    if let Ok(joined) = env::join_paths(new_paths) {
        env::set_var("PATH", joined);
    }
}

pub fn init_effective_path() {
    PATH_INIT.call_once(|| {
        refresh_effective_path();
    });
}

pub fn invalidate_arkcli_cache() {
    if let Ok(mut guard) = ARKCLI_BIN.lock() {
        *guard = None;
    }
}

fn cached_arkcli_binary() -> Option<PathBuf> {
    let mut guard = ARKCLI_BIN.lock().ok()?;
    if guard.is_none() {
        *guard = Some(find_arkcli_binary());
    }
    guard.clone().flatten()
}

pub fn find_arkcli_binary() -> Option<PathBuf> {
    let mut candidate_dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            let home_path = Path::new(&home);
            let nvm_dir = home_path.join(".nvm/versions/node");
            if nvm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                    let mut node_versions: Vec<PathBuf> = Vec::new();
                    for entry in entries.flatten() {
                        let bin_path = entry.path().join("bin");
                        if bin_path.exists() {
                            node_versions.push(bin_path);
                        }
                    }
                    node_versions.sort();
                    node_versions.reverse();
                    for nv in node_versions {
                        candidate_dirs.push(nv);
                    }
                }
            }
            candidate_dirs.push(home_path.join(".npm-global/bin"));
            candidate_dirs.push(home_path.join(".local/bin"));
        }
        candidate_dirs.push(PathBuf::from("/opt/homebrew/bin"));
        candidate_dirs.push(PathBuf::from("/usr/local/bin"));
        candidate_dirs.push(PathBuf::from("/usr/bin"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            candidate_dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if let Ok(localappdata) = env::var("LOCALAPPDATA") {
            candidate_dirs.push(PathBuf::from(localappdata).join("Programs").join("nodejs"));
        }
        candidate_dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
        candidate_dirs.push(PathBuf::from(r"C:\Program Files (x86)\nodejs"));
    }

    if let Ok(current_path) = env::var("PATH") {
        for p in env::split_paths(&current_path) {
            if !candidate_dirs.contains(&p) {
                candidate_dirs.push(p);
            }
        }
    }

    for dir in candidate_dirs {
        #[cfg(target_os = "windows")]
        {
            let cmd_path = dir.join("arkcli.cmd");
            if cmd_path.is_file() {
                return Some(cmd_path);
            }
            let exe_path = dir.join("arkcli.exe");
            if exe_path.is_file() {
                return Some(exe_path);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let bin_path = dir.join("arkcli");
            if bin_path.exists() {
                return Some(bin_path);
            }
        }
    }

    None
}

pub fn create_command(cmd: &str) -> Command {
    init_effective_path();

    let target_cmd = if cmd == "arkcli" {
        cached_arkcli_binary()
            .map(|bin| bin.to_string_lossy().to_string())
            .unwrap_or_else(|| cmd.to_string())
    } else {
        cmd.to_string()
    };

    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW, ensures cmd.exe never pops up a black console window!
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut c = Command::new("cmd.exe");
        c.creation_flags(CREATE_NO_WINDOW);
        c.arg("/C");
        // `cmd /C <路径>` 遇到路径里的空格就把空格当命令结束——arkcli 装在
        // `C:\Users\First Last\AppData\Roaming\npm\` 这类目录下时永远跑不起来，
        // 用量也就永远刷不出来。用 raw_arg 原样写入（std 的 arg 会按 MSVCRT
        // 规则把引号二次转义），cmd 自己会按「/C 后以引号开头」的规则剥掉这层引号。
        if target_cmd.contains(' ') {
            c.raw_arg(format!("\"{}\"", target_cmd));
        } else {
            c.arg(&target_cmd);
        }
        c
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(target_cmd)
    }
}

pub fn execute_cmd(cmd: &str, args: &[&str]) -> Result<Output, String> {
    execute_cmd_timeout(cmd, args, Duration::from_secs(20))
}

pub fn execute_cmd_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Result<Output, String> {
    let mut command = create_command(cmd);
    command.args(args);
    command
        .env("ARKCLI_NO_UPDATE_NOTIFIER", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("无法执行命令 '{}': {}", cmd, e))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stdout_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stderr_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_thread.join().unwrap_or_default();
                let stderr = stderr_thread.join().unwrap_or_default();
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    #[cfg(target_os = "windows")]
                    {
                        let pid = child.id();
                        let _ = Command::new("taskkill")
                            .args(["/F", "/T", "/PID", &pid.to_string()])
                            .creation_flags(0x08000000)
                            .status();
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = child.kill();
                    }
                    let _ = child.wait();
                    // 子进程可能留下持有继承管道写端的孙进程，此时 read_to_end 不会返回，
                    // join 会把调用方一起拖死（连超时错误都返回不了），故这里放弃回收。
                    drop(stdout_thread);
                    drop(stderr_thread);
                    return Err(format!("命令 '{}' 超时 ({}s)", cmd, timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("等待命令 '{}' 失败: {}", cmd, e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_effective_path() {
        init_effective_path();
        let path = env::var("PATH").unwrap_or_default();
        assert!(!path.is_empty(), "PATH should not be empty");
    }
}
