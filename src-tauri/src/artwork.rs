use std::path::PathBuf;

use crate::paths;

pub fn steam_cover_url(appid: &str) -> String {
    format!(
        "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.jpg",
        appid
    )
}

pub async fn sgdb_grid(game_id: &str, api_key: &str) -> Option<String> {
    if api_key.trim().is_empty() {
        return None;
    }
    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let response = client
        .get(format!(
            "https://www.steamgriddb.com/api/v2/grids/steam/{}",
            game_id
        ))
        .bearer_auth(api_key)
        .query(&[(
            "styles",
            "standard,alternate,blurred,white_logo,material,no_logo",
        )])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let data: serde_json::Value = response.json().await.ok()?;
    if data.get("success").and_then(|s| s.as_bool()) != Some(true) {
        return None;
    }
    for grid in data.get("data")?.as_array()? {
        if let Some(url) = grid.get("url").and_then(|u| u.as_str()) {
            return Some(url.to_string());
        }
    }
    None
}

pub fn cache_local_image(source: &str, game_id: &str) -> Option<String> {
    let src = PathBuf::from(source);
    if !src.is_file() {
        return None;
    }
    let ext = src
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dest = paths::art_cache_dir().join(format!("{}{}", game_id, ext));
    std::fs::create_dir_all(paths::art_cache_dir()).ok()?;
    std::fs::copy(&src, &dest).ok()?;
    Some(dest.to_string_lossy().to_string())
}

pub async fn download_steam_artwork(appid: &str, game_id: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;
    let response = client.get(steam_cover_url(appid)).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    let dest = paths::art_cache_dir().join(format!("{}.jpg", game_id));
    std::fs::create_dir_all(paths::art_cache_dir()).ok()?;
    std::fs::write(&dest, &bytes).ok()?;
    Some(dest.to_string_lossy().to_string())
}

fn which(binary: &str) -> bool {
    std::env::var("PATH")
        .map(|paths| {
            paths
                .split(':')
                .any(|dir| std::path::Path::new(dir).join(binary).exists())
        })
        .unwrap_or(false)
}

pub fn extract_exe_icon(exe_path: &str, game_id: &str) -> Option<String> {
    let target = paths::art_cache_dir().join(format!("{}.png", game_id));
    if target.exists() {
        return Some(target.to_string_lossy().to_string());
    }
    if !std::path::Path::new(exe_path).is_file() || !which("wrestool") || !which("icotool") {
        return None;
    }
    std::fs::create_dir_all(paths::art_cache_dir()).ok()?;

    let ico = paths::art_cache_dir().join(format!("{}.ico", game_id));
    let extracted = std::process::Command::new("wrestool")
        .args(["-x", "-t", "14", exe_path, "-o"])
        .arg(&ico)
        .status()
        .ok()?
        .success();
    if !extracted {
        return None;
    }
    let converted = std::process::Command::new("icotool")
        .arg("-x")
        .arg("-o")
        .arg(paths::art_cache_dir())
        .arg(&ico)
        .status()
        .ok()?
        .success();
    let _ = std::fs::remove_file(&ico);
    if !converted {
        return None;
    }

    let prefix = format!("{}_", game_id);
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(paths::art_cache_dir())
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with(&prefix) && name.ends_with(".png")
                })
                .unwrap_or(false)
        })
        .collect();
    candidates.sort_by_key(|path| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    let biggest = candidates.pop()?;
    std::fs::rename(&biggest, &target).ok()?;
    for leftover in candidates {
        let _ = std::fs::remove_file(leftover);
    }
    Some(target.to_string_lossy().to_string())
}

pub async fn search_steam_appid(game_name: &str) -> Option<String> {
    let cache_file = paths::art_cache_dir().join("steam_applist.json");
    let fresh = std::fs::metadata(&cache_file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .map(|age| age.as_secs() < 7 * 24 * 3600)
        .unwrap_or(false);

    if !fresh {
        let client = reqwest::Client::builder()
            .user_agent("GameHub")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .ok()?;
        let response = client
            .get("https://api.steampowered.com/ISteamApps/GetAppList/v2/")
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let text = response.text().await.ok()?;
        std::fs::create_dir_all(paths::art_cache_dir()).ok()?;
        let _ = std::fs::write(&cache_file, text);
    }

    let text = std::fs::read_to_string(&cache_file).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    let apps = data.get("applist")?.get("apps")?.as_array()?;
    let term = game_name.to_lowercase().trim().to_string();
    if term.is_empty() {
        return None;
    }
    for app in apps {
        let name = app.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name.to_lowercase() == term {
            return app.get("appid").map(|id| id.to_string());
        }
    }
    for app in apps {
        let name = app.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name.to_lowercase().contains(&term) {
            return app.get("appid").map(|id| id.to_string());
        }
    }
    None
}
