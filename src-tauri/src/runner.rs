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
        command.process_group(0);
        return command
            .spawn()
            .map_err(|e| format!("Failed to launch {}: {}", game.name, e));
    }

    let proton_path = match game.proton_version.as_ref().filter(|v| !v.trim().is_empty()) {
        Some(version) => proton::installed_versions()
            .get(version)
            .cloned()
            .ok_or_else(|| format!("Proton version '{}' is not installed", version))?,
        None => proton::resolve(settings).ok_or_else(|| {
            "No Proton versions found. Install one in Settings.".to_string()
        })?,
    };

    let prefix = paths::prefix_dir(&game.id);
    std::fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;

    let mut command = Command::new(&proton_path);
    command.arg("run").arg(&game.path).args(&extra);
    command.env(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        paths::home().join(".steam/steam"),
    );
    command.env("STEAM_COMPAT_DATA_PATH", &prefix);
    if let Some(overrides) = game
        .dll_overrides
        .as_ref()
        .filter(|o| !o.trim().is_empty())
    {
        let existing = std::env::var("WINEDLLOVERRIDES").unwrap_or_default();
        let merged = if existing.is_empty() {
            overrides.clone()
        } else {
            format!("{};{}", existing, overrides)
        };
        command.env("WINEDLLOVERRIDES", merged);
    }

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

pub fn find_pid_heroic(game_id: &str, game_name: &str, exclude: &[i32]) -> Option<i32> {
    let lower_name = game_name.to_lowercase();
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
        let lower = joined.to_lowercase();
        if lower.contains("legendary") && joined.contains(game_id) {
            return Some(pid);
        }
        if !lower_name.is_empty() && lower.contains("wine") && lower.contains(&lower_name) {
            return Some(pid);
        }
    }
    None
}
