use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::paths;

const CACHE_TTL_SECONDS: i64 = 7 * 24 * 3600;

fn cache_path() -> PathBuf {
    paths::cache_dir().join("protondb_cache.json")
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn load_cache() -> HashMap<String, Value> {
    if let Ok(text) = std::fs::read_to_string(cache_path()) {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
            return map.into_iter().collect();
        }
    }
    HashMap::new()
}

fn save_cache(cache: &HashMap<String, Value>) {
    std::fs::create_dir_all(paths::cache_dir()).ok();
    if let Ok(text) = serde_json::to_string(cache) {
        let _ = std::fs::write(cache_path(), text);
    }
}

pub async fn get_tier(appid: &str) -> Option<String> {
    if appid.trim().is_empty() {
        return None;
    }
    let mut cache = load_cache();
    if let Some(entry) = cache.get(appid) {
        let timestamp = entry
            .get("ts")
            .or_else(|| entry.get("timestamp"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        let tier = entry
            .get("tier")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if now() - timestamp < CACHE_TTL_SECONDS {
            if tier.is_empty() || tier == "unknown" {
                return None;
            }
            return Some(tier);
        }
    }

    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let response = client
        .get(format!(
            "https://www.protondb.com/api/v1/reports/summaries/{}.json",
            appid
        ))
        .send()
        .await
        .ok()?;
    if response.status().is_success() {
        if let Ok(data) = response.json::<Value>().await {
            if let Some(tier) = data.get("tier").and_then(|t| t.as_str()) {
                cache.insert(appid.to_string(), json!({ "tier": tier, "ts": now() }));
                save_cache(&cache);
                return Some(tier.to_string());
            }
        }
    }

    cache.insert(appid.to_string(), json!({ "tier": "unknown", "ts": now() }));
    save_cache(&cache);
    None
}
