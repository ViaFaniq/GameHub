use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

use crate::config::ManualGame;
use crate::paths;
use crate::proton;

pub fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            },
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn steam_runtime_entry() -> Option<std::path::PathBuf> {
    let home = paths::home();
    let bases = [
        home.join(".steam/steam/steamapps/common"),
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common"),
    ];
    for runtime in [
        "SteamLinuxRuntime_4",
        "SteamLinuxRuntime_sniper",
        "SteamLinuxRuntime_soldier",
    ] {
        for base in &bases {
            let entry = base.join(runtime).join("_v2-entry-point");
            if entry.is_file() {
                return Some(entry);
            }
        }
    }
    None
}

pub fn spawn_manual(game: &ManualGame, settings: &HashMap<String, String>) -> Result<Child, String> {
    let args_text = if game.use_global_args {
        settings
            .get("global_arguments")
            .cloned()
            .unwrap_or_default()
    } else {
        game.arguments.clone().unwrap_or_default()
    };
    let extra = split_args(&args_text);

    if game.runner_type == "native" {
        let mut command = Command::new(&game.path);
        command.args(&extra);
        if let Some(parent) = std::path::Path::new(&game.path).parent() {
            if !parent.as_os_str().is_empty() {
                command.current_dir(parent);
            }
        }
        command.process_group(0);
        return command
            .spawn()
            .map_err(|e| format!("Failed to launch {}: {}", game.name, e));
    }

    let proton_path = match game.proton_version.as_ref().filter(|v| !v.trim().is_empty()) {
        Some(version) => {
            let installed = proton::installed_versions();
            installed.get(version).cloned().ok_or_else(|| {
                if installed.is_empty() {
                    format!("Proton version '{}' is not installed. No Proton installed.", version)
                } else {
                    format!(
                        "Proton version '{}' is not installed. Available: {}",
                        version,
                        installed.keys().cloned().collect::<Vec<_>>().join(", ")
                    )
                }
            })?
        }
        None => proton::resolve(settings).ok_or_else(|| {
            "No Proton versions found. Install one in Settings.".to_string()
        })?,
    };

    let prefix = paths::prefix_dir(&game.id);
    std::fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;

    let exe_path = std::path::Path::new(&game.path);
    let cwd = exe_path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();

    let mut command = if let Some(entry) = steam_runtime_entry() {
        let mut command = Command::new(entry);
        command
            .arg("--verb=waitforexitandrun")
            .arg("--")
            .arg(&proton_path)
            .arg("waitforexitandrun")
            .arg(&game.path)
            .args(&extra);
        command
    } else {
        let mut command = Command::new(&proton_path);
        command
            .arg("waitforexitandrun")
            .arg(&game.path)
            .args(&extra);
        command
    };

    if !cwd.as_os_str().is_empty() {
        command.current_dir(&cwd);
    }
    command.env(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        paths::home().join(".steam/steam"),
    );
    command.env("STEAM_COMPAT_DATA_PATH", &prefix);
    command.env("PROTON_LOG", "1");
    if let Some(steam_id) = game
        .steam_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        command.env("SteamAppId", steam_id);
        command.env("SteamGameId", steam_id);
    }
    // Cracked / non-Steam games ship their own steam_api shim (Goldberg, RUNE, ...).
    // Wine's lsteamclient bridge conflicts with it and makes such games crash on
    // startup, so disable it the same way UMU does for non-Steam launches.
    let existing = std::env::var("WINEDLLOVERRIDES").unwrap_or_default();
    let mut overrides = if existing.is_empty() {
        "lsteamclient=d".to_string()
    } else {
        format!("{};lsteamclient=d", existing)
    };
    if let Some(extra) = game
        .dll_overrides
        .as_ref()
        .filter(|o| !o.trim().is_empty())
    {
        overrides.push(';');
        overrides.push_str(extra);
    }
    command.env("WINEDLLOVERRIDES", overrides);

    command.process_group(0);
    command
        .spawn()
        .map_err(|e| format!("Failed to launch {}: {}", game.name, e))
}

pub fn spawn_uri(uri: &str) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(uri)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open {}: {}", uri, e))
}

pub fn find_pid_by_env(key: &str, value: &str, exclude: &[i32]) -> Option<i32> {
    let needle = format!("{}={}", key, value);
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if exclude.contains(&pid) {
            continue;
        }
        let Ok(environ) = std::fs::read(format!("/proc/{}/environ", pid)) else {
            continue;
        };
        for variable in environ.split(|byte| *byte == 0) {
            if variable == needle.as_bytes() {
                return Some(pid);
            }
        }
    }
    None
}

pub fn find_pid_heroic(game_id: &str, _game_name: &str, exclude: &[i32]) -> Option<i32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if exclude.contains(&pid) {
            continue;
        }
        let Ok(cmdline) = std::fs::read(format!("/proc/{}/cmdline", pid)) else {
            continue;
        };
        let joined = cmdline
            .split(|byte| *byte == 0)
            .map(|part| String::from_utf8_lossy(part).to_string())
            .collect::<Vec<String>>()
            .join(" ");
        if joined.contains(game_id) {
            return Some(pid);
        }
    }
    None
}
