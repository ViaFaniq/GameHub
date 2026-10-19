use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::paths;

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct ManualGame {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub runner_type: String,
    pub proton_version: Option<String>,
    #[serde(default = "default_true")]
    pub use_global_args: bool,
    pub arguments: Option<String>,
    pub artwork: Option<String>,
    pub dll_overrides: Option<String>,
    pub steam_id: Option<String>,
    pub playtime: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct GamesData {
    pub games: Vec<ManualGame>,
    pub hidden_games: Vec<Value>,
}

fn settings_path() -> PathBuf {
    paths::config_dir().join("configuration.conf")
}

fn games_path() -> PathBuf {
    paths::config_dir().join("games.json")
}

fn steam_meta_path() -> PathBuf {
    paths::config_dir().join("steam_metadata.json")
}

fn heroic_meta_path() -> PathBuf {
    paths::config_dir().join("heroic_artwork.json")
}

fn playtime_path(kind: &str) -> PathBuf {
    paths::config_dir().join(format!("{}_playtime.json", kind))
}

pub fn load_settings() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(settings_path()) {
        let mut in_section = false;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case("[Settings]");
                continue;
            }
            if in_section {
                if let Some((key, value)) = line.split_once('=') {
                    map.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }
    map
}

pub fn save_settings(map: &HashMap<String, String>) {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let mut out = String::from("[Settings]\n");
    for key in keys {
        out.push_str(key);
        out.push_str(" = ");
        out.push_str(map.get(key).map(|v| v.as_str()).unwrap_or(""));
        out.push('\n');
    }
    let _ = std::fs::write(settings_path(), out);
}

pub fn load_games() -> GamesData {
    if let Ok(text) = std::fs::read_to_string(games_path()) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if value.is_array() {
                if let Ok(games) = serde_json::from_value::<Vec<ManualGame>>(value) {
                    return GamesData {
                        games,
                        hidden_games: vec![],
                    };
                }
            } else if let Ok(data) = serde_json::from_value::<GamesData>(value) {
                return data;
            }
        }
    }
    GamesData::default()
}

pub fn save_games(data: &GamesData) {
    if let Ok(text) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(games_path(), text);
    }
}

pub fn add_manual_game(mut game: ManualGame) -> ManualGame {
    let mut data = load_games();
    if game.id.is_empty() || data.games.iter().any(|g| g.id == game.id) {
        let mut id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        while data.games.iter().any(|g| g.id == id.to_string()) {
            id += 1;
        }
        game.id = id.to_string();
    }
    game.kind = "manual".to_string();
    data.games.push(game.clone());
    save_games(&data);
    game
}

pub fn update_manual_game(game: ManualGame) {
    let mut data = load_games();
    for existing in data.games.iter_mut() {
        if existing.id == game.id {
            let playtime = existing.playtime;
            let artwork = existing.artwork.clone();
            let steam_id = existing.steam_id.clone();
            *existing = game.clone();
            existing.playtime = playtime;
            if existing.artwork.is_none() {
                existing.artwork = artwork;
            }
            if existing.steam_id.is_none() {
                existing.steam_id = steam_id;
            }
            break;
        }
    }
    save_games(&data);
}

pub fn delete_manual_game(id: &str) {
    let mut data = load_games();
    data.games.retain(|g| g.id != id);
    save_games(&data);
}

pub fn set_manual_artwork(id: &str, artwork: Option<String>, steam_id: Option<String>) {
    let mut data = load_games();
    for game in data.games.iter_mut() {
        if game.id == id {
            game.artwork = artwork.clone();
            if steam_id.is_some() {
                game.steam_id = steam_id.clone();
            }
            break;
        }
    }
    save_games(&data);
}

fn load_json_object(path: &std::path::Path) -> Map<String, Value> {
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
            return map;
        }
    }
    Map::new()
}

fn save_json_object(path: &std::path::Path, map: &Map<String, Value>) {
    if let Ok(text) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(path, text);
    }
}

fn meta_get(path: PathBuf, id: &str) -> (Option<String>, Option<String>) {
    let map = load_json_object(&path);
    if let Some(entry) = map.get(id) {
        if let Value::String(s) = entry {
            return (Some(s.clone()), None);
        }
        let artwork = entry
            .get("artwork")
            .and_then(|a| a.as_str())
            .map(|s| s.to_string());
        let steam_id = entry
            .get("steam_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        return (artwork, steam_id);
    }
    (None, None)
}

fn meta_set(path: PathBuf, id: &str, artwork: Option<String>, steam_id: Option<String>) {
    let mut map = load_json_object(&path);
    let entry = map
        .entry(id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    if let Value::Object(obj) = entry {
        match artwork {
            Some(path) => {
                obj.insert("artwork".into(), Value::String(path));
            }
            None => {
                obj.remove("artwork");
            }
        }
        if let Some(sid) = steam_id {
            obj.insert("steam_id".into(), Value::String(sid));
        }
        if obj.is_empty() {
            map.remove(id);
        }
    }
    save_json_object(&path, &map);
}

pub fn steam_meta_get(id: &str) -> (Option<String>, Option<String>) {
    meta_get(steam_meta_path(), id)
}

pub fn steam_meta_set(id: &str, artwork: Option<String>, steam_id: Option<String>) {
    meta_set(steam_meta_path(), id, artwork, steam_id);
}

pub fn heroic_meta_get(id: &str) -> (Option<String>, Option<String>) {
    meta_get(heroic_meta_path(), id)
}

pub fn heroic_meta_set(id: &str, artwork: Option<String>, steam_id: Option<String>) {
    meta_set(heroic_meta_path(), id, artwork, steam_id);
}

fn load_json_numbers(path: PathBuf) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
            for (key, value) in map {
                if let Some(n) = value.as_i64() {
                    out.insert(key, n);
                }
            }
        }
    }
    out
}

fn save_json_numbers(path: PathBuf, map: &HashMap<String, i64>) {
    if let Ok(text) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(path, text);
    }
}

pub fn steam_playtime_map() -> HashMap<String, i64> {
    load_json_numbers(playtime_path("steam"))
}

pub fn sync_steam_playtime(api: &HashMap<String, i64>) {
    let mut cache = load_json_numbers(playtime_path("steam"));
    let mut changed = false;
    for (appid, seconds) in api {
        let current = cache.get(appid).copied().unwrap_or(0);
        if *seconds > current {
            cache.insert(appid.clone(), *seconds);
            changed = true;
        }
    }
    if changed {
        save_json_numbers(playtime_path("steam"), &cache);
    }
}

pub fn get_playtime(kind: &str, id: &str) -> i64 {
    match kind {
        "manual" => load_games()
            .games
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.playtime)
            .unwrap_or(0),
        "steam" | "heroic" => load_json_numbers(playtime_path(kind))
            .get(id)
            .copied()
            .unwrap_or(0),
        _ => 0,
    }
}

pub fn add_playtime(kind: &str, id: &str, seconds: i64) {
    if seconds <= 0 {
        return;
    }
    match kind {
        "manual" => {
            let mut data = load_games();
            for game in data.games.iter_mut() {
                if game.id == id {
                    game.playtime += seconds;
                    break;
                }
            }
            save_games(&data);
        }
        "steam" | "heroic" => {
            let mut cache = load_json_numbers(playtime_path(kind));
            let current = cache.get(id).copied().unwrap_or(0);
            cache.insert(id.to_string(), current + seconds);
            save_json_numbers(playtime_path(kind), &cache);
        }
        _ => {}
    }
}
