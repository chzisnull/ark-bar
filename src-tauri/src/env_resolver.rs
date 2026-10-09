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

// ---------------------------------------------------------------------------
// 浏览器拦截 shim 目录
// ---------------------------------------------------------------------------

/// no-op opener 脚本。`__LOG__` 会被替换成日志绝对路径。
/// 被拦下的 URL 记到日志里，用户排查「为什么用量一直刷不出来」时能看到证据。
const BROWSER_GUARD_SCRIPT: &str = r#"#!/bin/sh
# ArkBar 自动生成的浏览器拦截器：CLI 想弹浏览器时落到这里，只记日志、不跳转。
# 删除本文件不影响使用（下次轮询会重建）。
LOG="__LOG__"
if [ -f "$LOG" ]; then
  tail -c 32768 "$LOG" > "$LOG.$$.tmp" 2>/dev/null && mv "$LOG.$$.tmp" "$LOG"
fi
{
  printf '%s blocked-opener=%s argv:' "$(date '+%Y-%m-%d %H:%M:%S')" "$0"
  for a in "$@"; do printf ' %s' "$a"; done
  printf '\n'
} >> "$LOG" 2>/dev/null
exit 0
"#;

/// None = 还没成功算过。只缓存成功结果：一次性失败（HOME 缺失、目录建不出来）
/// 不该让拦截在整个进程生命周期里沉默关闭。
static BROWSER_GUARD_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static BROWSER_GUARD_WARNED: Once = Once::new();

fn browser_guard_warn(msg: &str) {
    BROWSER_GUARD_WARNED.call_once(|| crate::applog::log(msg));
}

/// 允许测试/排障指定 shim 目录，避免动到真实用户数据目录。
const BROWSER_GUARD_DIR_ENV: &str = "ARKBAR_BROWSER_GUARD_DIR";

fn default_browser_guard_dir() -> Option<PathBuf> {
    if let Some(custom) = env::var_os(BROWSER_GUARD_DIR_ENV) {
        if !custom.is_empty() {
            return Some(PathBuf::from(custom));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 上 agy 走 rundll32/`cmd /c start`，放同名 shim 也拦不住，直接不启用。
        None
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join("Library/Application Support/com.chzisnull.arkbar")
                .join("browser-guard"),
        )
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Ok(xdg) = env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("com.chzisnull.arkbar").join("browser-guard"));
            }
        }
        let home = env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join(".local/share/com.chzisnull.arkbar")
                .join("browser-guard"),
        )
    }
}

/// 把绝对路径塞进 sh 的双引号字面量里。`$` 与反引号必须一起转义，
/// 否则 HOME 里带 `$(`/反引号时脚本会执行命令替换。
fn shell_double_quote_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        if matches!(ch, '\\' | '"' | '$' | '`') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(unix)]
fn guard_scripts_ready(dir: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    ["open", "xdg-open"].iter().all(|name| {
        std::fs::metadata(dir.join(name))
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

#[cfg(not(unix))]
fn guard_scripts_ready(dir: &Path) -> bool {
    dir.join("open").is_file()
}

fn write_browser_guard_script(path: &Path, body: &str) -> bool {
    let same = std::fs::read_to_string(path)
        .map(|cur| cur == body)
        .unwrap_or(false);
    if !same && std::fs::write(path, body).is_err() {
        return false;
    }
    // 内容没变也要确认可执行位：被备份/还原工具 strip 掉后，
    // shell 会跳过不可执行的 shim 继续找到真正的 open。
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let has_exec = std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if !has_exec
            && std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_err()
        {
            return false;
        }
    }
    true
}

/// 返回（并按需创建）shim 目录；不可用时返回 None，调用方照常执行命令。
pub fn browser_guard_dir() -> Option<PathBuf> {
    if let Ok(guard) = BROWSER_GUARD_DIR.lock() {
        if let Some(dir) = guard.as_ref() {
            // shim 被删掉或被去掉可执行位时重新生成，不然会静默退回系统 opener。
            if guard_scripts_ready(dir) {
                return Some(dir.clone());
            }
        }
    }

    let computed = default_browser_guard_dir().and_then(|dir| {
        if std::fs::create_dir_all(&dir).is_err() {
            browser_guard_warn(&format!(
                "浏览器拦截：shim 目录 {} 创建失败，agy 自动弹登录页的保护已关闭",
                dir.display()
            ));
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // 目录在 PATH 最前面，收紧权限避免被别人塞东西进来。
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }

        let log_path = dir.join("blocked-openers.log");
        let log_literal = shell_double_quote_escape(&log_path.to_string_lossy());
        let body = BROWSER_GUARD_SCRIPT.replace("__LOG__", &log_literal);

        let mut ok = true;
        for name in ["open", "xdg-open"] {
            ok &= write_browser_guard_script(&dir.join(name), &body);
        }
        if ok {
            Some(dir)
        } else {
            browser_guard_warn(&format!(
                "浏览器拦截：shim 写入 {} 失败，agy 自动弹登录页的保护已关闭",
                dir.display()
            ));
            None
        }
    });

    if let Some(ref dir) = computed {
        if let Ok(mut guard) = BROWSER_GUARD_DIR.lock() {
            *guard = Some(dir.clone());
        }
    }
    computed
}

/// `since` 之后是否拦下过浏览器打开。调用方用它把「agy 判定未登录」和
/// 「agy 根本没装」区分开——前者要提示用户去终端重新登录。
pub fn browser_guard_blocked_since(since: std::time::SystemTime) -> bool {
    let Some(dir) = browser_guard_dir() else {
        return false;
    };
    std::fs::metadata(dir.join("blocked-openers.log"))
        .and_then(|m| m.modified())
        .map(|modified| modified >= since)
        .unwrap_or(false)
}

pub fn execute_cmd(cmd: &str, args: &[&str]) -> Result<Output, String> {
    execute_cmd_timeout(cmd, args, Duration::from_secs(20))
}

pub fn execute_cmd_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Result<Output, String> {
    execute_cmd_timeout_inner(cmd, args, timeout, false)
}

/// 给「后台轮询、绝不能打扰用户」的命令用：拦掉子进程打开浏览器的能力。
///
/// 背景：agy（Antigravity CLI）在 token 刷新遇到瞬时网络错误时会判定「未登录」，
/// 直接降级成交互式 OAuth 并调用 `open <accounts.google.com/...>` 弹出授权页——
/// 后台每几分钟一次的 `agy -p /usage` 轮询因此会凭空弹登录页（用户明明已登录）。
/// agy 的 opener 走 PATH 查找（`open` / `xdg-open`），且没有 --no-browser 开关，
/// 所以这里把 PATH 最前面插进一个 no-op shim 目录，顺带把 stdin 接到 null。
///
/// 只用于用法查询这类无人工交互的调用；像 `arkcli auth login` 这种本就该开浏览器
/// 的交互式登录不要用它。
pub fn execute_cmd_timeout_no_browser(
    cmd: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<Output, String> {
    execute_cmd_timeout_inner(cmd, args, timeout, true)
}

fn execute_cmd_timeout_inner(
    cmd: &str,
    args: &[&str],
    timeout: Duration,
    block_browser: bool,
) -> Result<Output, String> {
    let mut command = create_command(cmd);
    command.args(args);
    command
        .env("ARKCLI_NO_UPDATE_NOTIFIER", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if block_browser {
        // 子进程继承父进程（GUI app，无控制终端）的 stdin 只会让它挂起来等输入。
        command.stdin(Stdio::null());
        if let Some(dir) = browser_guard_dir() {
            let mut paths: Vec<PathBuf> = vec![dir];
            // 空段等于 CWD，对它没意义，一并滤掉。
            paths.extend(
                env::split_paths(&env::var("PATH").unwrap_or_default())
                    .filter(|p| !p.as_os_str().is_empty()),
            );
            match env::join_paths(paths) {
                Ok(joined) => {
                    command.env("PATH", joined);
                }
                // 静默失败会让拦截形同不存在（只有 PATH 含分隔符这种极端情况才会走到）。
                Err(e) => browser_guard_warn(&format!(
                    "浏览器拦截：PATH 注入失败（{e}），agy 自动弹登录页的保护对该次调用已关闭"
                )),
            }
        }
    }

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

    /// 用隔离目录跑，别碰真实用户数据目录里的日志和 shim。
    /// 覆盖变量保留到进程结束：清掉后重算会落回真实 HOME 目录。
    fn isolated_guard_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("arkbar-guard-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        env::set_var(BROWSER_GUARD_DIR_ENV, &dir);
        dir
    }

    #[cfg(unix)]
    #[test]
    fn test_browser_guard_blocks_open() {
        use std::os::unix::fs::PermissionsExt;

        let dir = isolated_guard_dir();
        let guard = browser_guard_dir().expect("guard dir should be creatable on unix");
        assert_eq!(guard, dir, "应使用 ARKBAR_BROWSER_GUARD_DIR 指定的目录");

        // 第一步只做解析，确认 open/xdg-open 落在 shim 上；第二步才真的调用它们，
        // 这样万一注入失效也只是断言失败，不会给跑测试的人弹出真实浏览器。
        let probe = |script: &str| {
            execute_cmd_timeout_no_browser("sh", &["-c", script], Duration::from_secs(10))
                .expect("guarded command should run")
        };

        let resolved: Vec<String> =
            String::from_utf8_lossy(&probe("command -v open; command -v xdg-open").stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .collect();
        assert_eq!(
            resolved,
            vec![
                dir.join("open").to_string_lossy().to_string(),
                dir.join("xdg-open").to_string_lossy().to_string(),
            ],
            "open/xdg-open 应解析到 shim"
        );

        let out = probe("open https://example.com/probe1; xdg-open https://example.com/probe2");
        assert_eq!(out.status.code(), Some(0), "shim 应以 exit 0 收场");

        let logged = std::fs::read_to_string(dir.join("blocked-openers.log")).unwrap_or_default();
        assert!(
            logged.contains(&format!("blocked-opener={}", dir.join("open").display()))
                && logged.contains("https://example.com/probe1"),
            "shim 应把被拦下的 URL 写进日志，实际: {logged}"
        );
        assert!(
            logged.contains(&format!("blocked-opener={}", dir.join("xdg-open").display()))
                && logged.contains("https://example.com/probe2"),
            "xdg-open shim 也应记录，实际: {logged}"
        );

        // 拦截的时间戳要被上层用来判断「这次调用触发了登录」。
        let before = std::time::SystemTime::now();
        probe("open https://example.com/probe3");
        assert!(
            browser_guard_blocked_since(before),
            "应在 since 之后检测到拦截"
        );
        assert!(
            !browser_guard_blocked_since(before + Duration::from_secs(120)),
            "未来时间点不应命中"
        );

        // 自愈 1：shim 被清理工具删掉后要能重建，否则会静默退回系统 opener。
        std::fs::remove_file(dir.join("open")).expect("remove shim");
        assert_eq!(
            browser_guard_dir().expect("guard dir should be regenerated"),
            dir,
            "重建后目录不应变"
        );
        assert!(dir.join("open").is_file(), "shim 应被重新生成");

        // 自愈 2：可执行位被剥掉（只查存在性的实现会在这里静默失效：
        // shell 会跳过不可执行的 shim，继续找到真正的 open）。
        std::fs::set_permissions(dir.join("open"), std::fs::Permissions::from_mode(0o644))
            .expect("strip exec bit");
        assert_eq!(
            browser_guard_dir().expect("guard dir should repair itself"),
            dir
        );
        let mode = std::fs::metadata(dir.join("open"))
            .expect("stat shim")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "可执行位应被补回");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_shell_double_quote_escape() {
        // 路径里的 `$`/反引号/引号必须在双引号字面量里失去特殊含义。
        assert_eq!(
            shell_double_quote_escape(r#"a"/tmp/x $(id) `id` b"#),
            r#"a\"/tmp/x \$(id) \`id\` b"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_shell_double_quote_escape_neutralizes_expansion() {
        let dir = env::temp_dir().join(format!("arkbar-escape-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        for (attack, marker) in [
            (format!("$(touch {}/pwned_subst)", dir.display()), "pwned_subst"),
            (format!("`touch {}/pwned_bt`", dir.display()), "pwned_bt"),
        ] {
            let escaped = shell_double_quote_escape(&attack);
            let out = Command::new("sh")
                .arg("-c")
                .arg(format!("printf '%s' \"{escaped}\""))
                .output()
                .expect("sh should run");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                attack,
                "转义后应原样输出，而不是执行命令替换"
            );
            assert!(
                !dir.join(marker).exists(),
                "{marker}: 命令替换被执行了，转义不完整"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
