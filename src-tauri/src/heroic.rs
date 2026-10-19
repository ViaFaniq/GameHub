use std::path::PathBuf;

use crate::model::Game;
use crate::paths;

fn find_config(subpath: &str) -> Option<PathBuf> {
    for root in paths::heroic_roots() {
        let candidate = root.join(subpath);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn scan() -> Vec<Game> {
    let mut games = scan_legendary();
    games.extend(scan_gog());
    games
}

fn scan_legendary() -> Vec<Game> {
    let mut games = Vec::new();
    let Some(path) = find_config("legendaryConfig/legendary/installed.json") else {
        return games;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return games;
    };
    let Ok(serde_json::Value::Object(entries)) = serde_json::from_str::<serde_json::Value>(&text)
    else {
        return games;
    };
    for (app_name, details) in entries {
        let name = details
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_string();
        games.push(Game {
            id: app_name.clone(),
            name,
            kind: "heroic".into(),
            installed: true,
            platform: Some("legendary".into()),
            command: Some(format!("heroic://launch/legendary/{}", app_name)),
            path: details
                .get("install_path")
                .and_then(|p| p.as_str())
                .map(|p| p.to_string()),
            ..Default::default()
        });
    }
    games
}

fn scan_gog() -> Vec<Game> {
    let mut games = Vec::new();
    let Some(path) = find_config("gog_store/installed.json") else {
        return games;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return games;
    };
    let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
        return games;
    };
    let Some(installed) = data.get("installed").and_then(|i| i.as_array()) else {
        return games;
    };
    for entry in installed {
        let app_id = entry
            .get("appName")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();
        if app_id.is_empty() {
            continue;
        }
        let name = entry
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_string();
        games.push(Game {
            id: app_id.clone(),
            name,
            kind: "heroic".into(),
            installed: true,
            platform: Some("gog".into()),
            command: Some(format!("heroic://launch/gog/{}", app_id)),
            path: entry
                .get("install_path")
                .and_then(|p| p.as_str())
                .map(|p| p.to_string()),
            ..Default::default()
        });
    }
    games
}
