use std::collections::HashMap;
use std::path::PathBuf;

use crate::paths;

pub fn installed_versions() -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let search_dirs = [
        paths::home().join(".steam/root/compatibilitytools.d"),
        paths::proton_install_dir(),
    ];
    for dir in search_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let folder = entry.file_name().to_string_lossy().to_string();
            let proton = entry.path().join("proton");
            if proton.exists() {
                versions.insert(folder, proton.to_string_lossy().to_string());
            }
        }
    }
    versions
}

fn resolve_custom(path: &str) -> Option<String> {
    let candidate = PathBuf::from(path);
    if !candidate.exists() {
        return None;
    }
    if candidate.file_name().map(|n| n == "proton").unwrap_or(false) {
        return Some(candidate.to_string_lossy().to_string());
    }
    let joined = candidate.join("proton");
    if joined.exists() {
        return Some(joined.to_string_lossy().to_string());
    }
    None
}

pub fn latest_version() -> Option<String> {
    let versions = installed_versions();
    let mut names: Vec<&String> = versions.keys().collect();
    names.sort();
    names.reverse();
    names.first().and_then(|name| versions.get(*name).cloned())
}

pub fn resolve(settings: &HashMap<String, String>) -> Option<String> {
    if let Some(custom) = settings
        .get("custom_proton_path")
        .filter(|value| !value.trim().is_empty())
    {
        if let Some(path) = resolve_custom(custom) {
            return Some(path);
        }
    }
    latest_version()
}
