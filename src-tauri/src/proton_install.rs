use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::paths;

#[derive(Serialize, Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub url: String,
    pub checksum_url: Option<String>,
}

pub async fn available_releases(source: &str) -> Result<Vec<Release>, String> {
    let repo = match source {
        "cachyos" => "CachyOS/proton-cachyos",
        _ => "GloriousEggroll/proton-ge-custom",
    };
    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{}/releases?per_page=30",
            repo
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub API error {}", response.status()));
    }
    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;

    let mut releases = Vec::new();
    for release in data.as_array().cloned().unwrap_or_default() {
        let tag = release
            .get("tag_name")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if tag.is_empty() {
            continue;
        }
        let assets = release
            .get("assets")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        let mut chosen: Option<(String, String)> = None;
        for asset in &assets {
            let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if !(name.ends_with(".tar.gz") || name.ends_with(".tar.xz")) {
                continue;
            }
            if name.contains("aarch64") || name.contains("arm64") {
                continue;
            }
            let url = asset
                .get("browser_download_url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let base = name
                .rsplit_once(".tar.")
                .map(|(b, _)| b.to_string())
                .unwrap_or_else(|| name.to_string());
            let is_x86 = name.contains("x86_64") || name.contains("amd64");
            let replace = match &chosen {
                None => true,
                Some((_, existing_base)) => {
                    is_x86
                        && !(existing_base.contains("x86_64") || existing_base.contains("amd64"))
                }
            };
            if replace {
                chosen = Some((url, base));
            }
        }

        if let Some((url, base)) = chosen {
            let checksum_name = format!("{}.sha512sum", base);
            let checksum_url = assets.iter().find_map(|asset| {
                let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name == checksum_name {
                    asset
                        .get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(|u| u.to_string())
                } else {
                    None
                }
            });
            releases.push(Release {
                tag,
                url,
                checksum_url,
            });
        }
    }
    Ok(releases)
}

fn emit_progress(app: &AppHandle, tag: &str, status: &str, downloaded: u64, total: u64) {
    let _ = app.emit(
        "proton-progress",
        serde_json::json!({
            "tag": tag,
            "status": status,
            "downloaded": downloaded,
            "total": total,
        }),
    );
}

pub async fn install(
    app: &AppHandle,
    tag: &str,
    url: &str,
    checksum_url: Option<String>,
) -> Result<(), String> {
    let dir = paths::proton_install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let extension = if url.ends_with(".tar.gz") {
        ".tar.gz"
    } else if url.ends_with(".tar.xz") {
        ".tar.xz"
    } else {
        return Err("Unsupported archive format".to_string());
    };
    let archive_path = dir.join(format!("{}{}", tag, extension));

    let client = reqwest::Client::builder()
        .user_agent("GameHub")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Download failed with status {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(&archive_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed().as_millis() >= 200 {
            last_emit = std::time::Instant::now();
            emit_progress(app, tag, "downloading", downloaded, total);
        }
    }
    let _ = file.flush().await;
    drop(file);
    emit_progress(app, tag, "downloading", downloaded, total);

    if let Some(checksum) = checksum_url {
        if let Err(error) = verify_checksum(&client, &checksum, &archive_path).await {
            let _ = std::fs::remove_file(&archive_path);
            return Err(format!("Checksum verification failed: {}", error));
        }
    }

    emit_progress(app, tag, "extracting", downloaded, total);
    let archive = archive_path.clone();
    let destination = dir.clone();
    tauri::async_runtime::spawn_blocking(move || extract(&archive, &destination))
        .await
        .map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(&archive_path);
    emit_progress(app, tag, "done", downloaded, total);
    Ok(())
}

async fn verify_checksum(
    client: &reqwest::Client,
    checksum_url: &str,
    archive: &Path,
) -> Result<(), String> {
    let text = client
        .get(checksum_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let expected = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    if expected.len() != 128 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(());
    }
    let file = archive.to_path_buf();
    let actual = tauri::async_runtime::spawn_blocking(move || sha512_file(&file))
        .await
        .map_err(|e| e.to_string())??;
    if actual == expected {
        Ok(())
    } else {
        Err("hash mismatch".to_string())
    }
}

fn sha512_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha512};
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha512::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(hex::encode(hasher.finalize()))
}

fn extract(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.ends_with(".tar.gz") {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(destination).map_err(|e| e.to_string())?;
    } else {
        let decoder = xz2::read::XzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(destination).map_err(|e| e.to_string())?;
    }
    Ok(())
}
