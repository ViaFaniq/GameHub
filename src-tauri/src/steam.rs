use std::collections::HashSet;
use std::path::PathBuf;

use crate::model::Game;
use crate::paths;
use crate::vdf;

const UNWANTED: [&str; 4] = [
    "steam linux runtime",
    "proton",
    "steamworks common redistributables",
    "steamvr",
];

fn is_unwanted(name: &str) -> bool {
    let lower = name.to_lowercase();
    UNWANTED.iter().any(|u| lower.contains(u))
}

pub fn libraries() -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in paths::steam_roots() {
        let apps = root.join("steamapps");
        if !apps.exists() {
            continue;
        }
        if !libraries.contains(&apps) {
            libraries.push(apps.clone());
        }
        if let Ok(text) = std::fs::read_to_string(apps.join("libraryfolders.vdf")) {
            let parsed = vdf::parse(&text);
            if let Some(folders) = parsed.get("libraryfolders").and_then(|v| v.entries()) {
                for entry in folders.values() {
                    let path = match entry.as_str() {
                        Some(p) => Some(p.to_string()),
                        None => entry
                            .get("path")
                            .and_then(|p| p.as_str())
                            .map(|p| p.to_string()),
                    };
                    if let Some(path) = path {
                        let extra = PathBuf::from(path).join("steamapps");
                        if extra.exists() && !libraries.contains(&extra) {
                            libraries.push(extra);
                        }
                    }
                }
            }
        }
    }
    libraries
}

pub fn scan_installed() -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for library in libraries() {
        let Ok(entries) = std::fs::read_dir(&library) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !(file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let parsed = vdf::parse(&text);
            let Some(app) = parsed.get("AppState") else {
                continue;
            };
            let appid = app.get("appid").and_then(|v| v.as_str()).unwrap_or("");
            let name = app.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let install_dir = app
                .get("installdir")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if appid.is_empty() || name.is_empty() || seen.contains(appid) || is_unwanted(name) {
                continue;
            }
            seen.insert(appid.to_string());
            let path = library.join("common").join(install_dir);
            games.push(Game {
                id: appid.to_string(),
                name: name.to_string(),
                kind: "steam".into(),
                installed: true,
                artwork: Some(crate::artwork::steam_cover_url(appid)),
                path: Some(path.to_string_lossy().to_string()),
                ..Default::default()
            });
        }
    }
    games
}

pub fn find_steam_id() -> Option<String> {
    for root in paths::steam_roots() {
        let path = root.join("config/loginusers.vdf");
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let parsed = vdf::parse(&text);
        let Some(users) = parsed.get("users").and_then(|v| v.entries()) else {
            continue;
        };
        let mut first: Option<String> = None;
        for (id, user) in users {
            if first.is_none() && id.chars().all(|c| c.is_ascii_digit()) {
                first = Some(id.clone());
            }
            let most_recent = user
                .get("MostRecent")
                .and_then(|v| v.as_str())
                .map(|v| v == "1")
                .unwrap_or(false);
            if most_recent && id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.clone());
            }
        }
        if first.is_some() {
            return first;
        }
    }
    None
}

pub async fn fetch_owned_games(api_key: &str, steam_id: &str) -> Result<Vec<Game>, String> {
    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get("https://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/")
        .query(&[
            ("key", api_key),
            ("steamid", steam_id),
            ("format", "json"),
            ("include_appinfo", "1"),
            ("include_played_free_games", "1"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Steam API error {}", response.status()));
    }
    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let mut games = Vec::new();
    if let Some(items) = data
        .get("response")
        .and_then(|r| r.get("games"))
        .and_then(|g| g.as_array())
    {
        for item in items {
            let appid = item
                .get("appid")
                .map(|a| a.to_string())
                .unwrap_or_default();
            let name = item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let playtime = item
                .get("playtime_forever")
                .and_then(|p| p.as_i64())
                .unwrap_or(0)
                * 60;
            if appid.is_empty() || name.is_empty() || is_unwanted(&name) {
                continue;
            }
            games.push(Game {
                id: appid.clone(),
                name,
                kind: "steam".into(),
                installed: false,
                artwork: Some(crate::artwork::steam_cover_url(&appid)),
                playtime,
                ..Default::default()
            });
        }
    }
    Ok(games)
}
