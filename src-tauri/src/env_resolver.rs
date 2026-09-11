use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn init_effective_path() {
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

pub fn execute_cmd(cmd: &str, args: &[&str]) -> Result<Output, String> {
    init_effective_path();

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg(cmd).args(args);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut c = Command::new(cmd);
        c.args(args);
        c
    };

    command
        .envs(vec![
            ("ARKCLI_NO_UPDATE_NOTIFIER", "1"),
            ("ARKCLI_CALLER_TYPE", "desktop_app"),
            ("ARKCLI_CALLER_NAME", "ark-bar"),
            ("ARKCLI_SKILL_NAME", "ark-bar"),
        ])
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
