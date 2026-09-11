use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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
    refresh_effective_path();
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
    refresh_effective_path();

    let target_cmd = if cmd == "arkcli" {
        if let Some(bin) = find_arkcli_binary() {
            bin.to_string_lossy().to_string()
        } else {
            cmd.to_string()
        }
    } else {
        cmd.to_string()
    };

    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW, ensures cmd.exe never pops up a black console window!
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut c = Command::new("cmd.exe");
        c.creation_flags(CREATE_NO_WINDOW);
        c.arg("/C").arg(target_cmd);
        c
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(target_cmd)
    }
}

pub fn execute_cmd(cmd: &str, args: &[&str]) -> Result<Output, String> {
    let mut command = create_command(cmd);
    command.args(args);
    command
        .env("ARKCLI_NO_UPDATE_NOTIFIER", "1")
        .output()
        .map_err(|e| format!("无法执行命令 '{}': {}", cmd, e))
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
