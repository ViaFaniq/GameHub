mod artwork;
mod config;
mod desktop_entry;
mod heroic;
mod model;
mod paths;
mod proton;
mod proton_install;
mod protondb;
mod runner;
mod steam;
mod vdf;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use model::{Game, LibraryPayload};

#[allow(dead_code)]
pub struct RunningEntry {
    pub pid: i32,
    pub name: String,
    pub kind: String,
    pub started: Instant,
}

#[derive(Default)]
pub struct AppState {
    pub running: Mutex<HashMap<String, RunningEntry>>,
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ManualGameInput {
    id: String,
    name: String,
    path: String,
    runner_type: String,
    proton_version: Option<String>,
    use_global_args: bool,
    arguments: Option<String>,
    artwork_type: String,
    artwork_value: Option<String>,
    dll_overrides: Option<String>,
}

fn is_true(value: Option<&String>) -> bool {
    value
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

async fn resolve_artwork(
    input_type: &str,
    input_value: Option<String>,
    game_id: &str,
) -> Result<(Option<String>, Option<String>), String> {
    match input_type {
        "file" => {
            let value = input_value.ok_or_else(|| "Missing artwork file".to_string())?;
            let path = artwork::cache_local_image(&value, game_id)
                .ok_or_else(|| "Could not copy artwork file".to_string())?;
            Ok((Some(path), None))
        }
        "steam" => {
            let appid = input_value.ok_or_else(|| "Missing Steam AppID".to_string())?;
            let path = artwork::download_steam_artwork(&appid, game_id).await;
            Ok((path, Some(appid)))
        }
        _ => Ok((None, None)),
    }
}

fn manual_as_game(manual: &config::ManualGame) -> Game {
    let has_user_artwork = manual.artwork.is_some();
    let artwork = match manual.artwork.clone() {
        Some(path) => Some(path),
        None => artwork::extract_exe_icon(&manual.path, &manual.id),
    };
    Game {
        id: manual.id.clone(),
        name: manual.name.clone(),
        kind: "manual".into(),
        installed: true,
        artwork_source: if artwork.is_some() {
            Some(if has_user_artwork { "user" } else { "exe" }.to_string())
        } else {
            None
        },
        artwork,
        path: Some(manual.path.clone()),
        playtime: manual.playtime,
        runner_type: Some(if manual.runner_type.is_empty() {
            "proton".into()
        } else {
            manual.runner_type.clone()
        }),
        proton_version: manual.proton_version.clone(),
        use_global_args: Some(manual.use_global_args),
        arguments: manual.arguments.clone(),
        dll_overrides: manual.dll_overrides.clone(),
        steam_id: manual.steam_id.clone(),
        ..Default::default()
    }
}

fn refresh_manual_entry(id: &str) {
    if !desktop_entry::exists("manual", id) {
        return;
    }
    if let Some(manual) = config::load_games()
        .games
        .into_iter()
        .find(|game| game.id == id)
    {
        let _ = desktop_entry::create(&manual_as_game(&manual));
    }
}

fn emit_game_started(app: &AppHandle, id: &str, kind: &str, name: &str) {
    let _ = app.emit(
        "game-started",
        json!({ "id": id, "kind": kind, "name": name }),
    );
}

fn register_and_wait(
    app: AppHandle,
    mut child: std::process::Child,
    id: String,
    kind: String,
    name: String,
) {
    let pid = child.id() as i32;
    {
        let state = app.state::<AppState>();
        let mut running = state.running.lock().unwrap();
        running.insert(
            id.clone(),
            RunningEntry {
                pid,
                name: name.clone(),
                kind: kind.clone(),
                started: Instant::now(),
            },
        );
    }
    emit_game_started(&app, &id, &kind, &name);
    std::thread::spawn(move || {
        let _ = child.wait();
        let seconds = {
            let state = app.state::<AppState>();
            let mut running = state.running.lock().unwrap();
            running
                .remove(&id)
                .map(|entry| entry.started.elapsed().as_secs() as i64)
                .unwrap_or(0)
        };
        if seconds > 0 {
            config::add_playtime(&kind, &id, seconds);
        }
        let _ = app.emit(
            "playtime-updated",
            json!({ "id": &id, "kind": &kind, "seconds": seconds }),
        );
        let _ = app.emit("game-stopped", json!({ "id": &id, "kind": &kind }));
    });
}

fn track_latent(app: AppHandle, id: String, kind: String, name: String) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut found_pid: Option<i32> = None;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1000));
            let exclude: Vec<i32> = {
                let state = app.state::<AppState>();
                let running = state.running.lock().unwrap();
                if running.contains_key(&id) {
                    return;
                }
                running.values().map(|entry| entry.pid).collect()
            };
            let pid = if kind == "steam" {
                runner::find_pid_by_env("SteamAppId", &id, &exclude)
            } else {
                runner::find_pid_heroic(&id, &name, &exclude)
            };
            if let Some(pid) = pid {
                found_pid = Some(pid);
                break;
            }
        }
        let Some(pid) = found_pid else {
            return;
        };
        {
            let state = app.state::<AppState>();
            let mut running = state.running.lock().unwrap();
            if running.contains_key(&id) {
                return;
            }
            running.insert(
                id.clone(),
                RunningEntry {
                    pid,
                    name: name.clone(),
                    kind: kind.clone(),
                    started: Instant::now(),
                },
            );
        }
        emit_game_started(&app, &id, &kind, &name);
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if !std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                break;
            }
        }
        let seconds = {
            let state = app.state::<AppState>();
            let mut running = state.running.lock().unwrap();
            running
                .remove(&id)
                .map(|entry| entry.started.elapsed().as_secs() as i64)
                .unwrap_or(0)
        };
        if seconds > 0 {
            config::add_playtime(&kind, &id, seconds);
        }
        let _ = app.emit(
            "playtime-updated",
            json!({ "id": &id, "kind": &kind, "seconds": seconds }),
        );
        let _ = app.emit("game-stopped", json!({ "id": &id, "kind": &kind }));
    });
}

#[tauri::command]
async fn scan_library(state: State<'_, AppState>) -> Result<LibraryPayload, String> {
    let settings = config::load_settings();

    let mut steam_games = steam::scan_installed();
    let mut api_playtime: HashMap<String, i64> = HashMap::new();

    let enable_api = is_true(settings.get("enable_steam_api"));
    let api_key = settings.get("steam_api_key").cloned().unwrap_or_default();
    if enable_api && !api_key.trim().is_empty() {
        if let Some(steam_id) = steam::find_steam_id() {
            if let Ok(owned) = steam::fetch_owned_games(api_key.trim(), &steam_id).await {
                let mut known: HashSet<String> =
                    steam_games.iter().map(|g| g.id.clone()).collect();
                for game in owned {
                    api_playtime.insert(game.id.clone(), game.playtime);
                    if !known.contains(&game.id) {
                        known.insert(game.id.clone());
                        steam_games.push(game);
                    }
                }
            }
        }
    }

    let steam_cache = config::steam_playtime_map();
    for game in steam_games.iter_mut() {
        let local = steam_cache.get(&game.id).copied().unwrap_or(0);
        let api = api_playtime.get(&game.id).copied().unwrap_or(0);
        game.playtime = local.max(api);
        let (artwork, steam_id) = config::steam_meta_get(&game.id);
        match artwork {
            Some(path) => {
                game.artwork = Some(path);
                game.artwork_source = Some("user".into());
            }
            None => {
                game.artwork_source = Some("steam".into());
            }
        }
        if let Some(sid) = steam_id {
            game.steam_id = Some(sid);
        }
    }
    config::sync_steam_playtime(&api_playtime);

    let mut heroic_games = heroic::scan();
    for game in heroic_games.iter_mut() {
        let (artwork, steam_id) = config::heroic_meta_get(&game.id);
        if let Some(path) = artwork {
            game.artwork = Some(path);
            game.artwork_source = Some("user".into());
        }
        if let Some(sid) = steam_id {
            if game.artwork.is_none() {
                game.artwork = Some(artwork::steam_cover_url(&sid));
                game.artwork_source = Some("steam".into());
            }
            game.steam_id = Some(sid);
        }
        game.playtime = config::get_playtime("heroic", &game.id);
    }

    let manual_games: Vec<Game> = config::load_games()
        .games
        .iter()
        .map(manual_as_game)
        .collect();

    let running: Vec<String> = state.running.lock().unwrap().keys().cloned().collect();
    let mut games = steam_games;
    games.extend(manual_games);
    games.extend(heroic_games);
    for game in games.iter_mut() {
        game.desktop_entry = desktop_entry::exists(&game.kind, &game.id);
    }

    Ok(LibraryPayload {
        games,
        running,
        protons: proton::installed_versions(),
        settings,
        errors: Vec::new(),
    })
}

#[tauri::command]
fn get_settings() -> HashMap<String, String> {
    config::load_settings()
}

#[tauri::command]
fn save_settings(settings: HashMap<String, String>) {
    config::save_settings(&settings);
}

#[tauri::command]
fn list_protons() -> HashMap<String, String> {
    proton::installed_versions()
}

#[tauri::command]
async fn proton_releases(source: String) -> Result<Vec<proton_install::Release>, String> {
    proton_install::available_releases(&source).await
}

#[tauri::command]
fn install_proton(
    app: AppHandle,
    tag: String,
    url: String,
    checksum_url: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        let result = proton_install::install(&app, &tag, &url, checksum_url).await;
        let payload = match result {
            Ok(()) => json!({ "tag": &tag, "ok": true }),
            Err(error) => json!({ "tag": &tag, "ok": false, "error": error }),
        };
        let _ = app.emit("proton-install-done", payload);
    });
    Ok(())
}

#[tauri::command]
async fn add_manual_game(input: ManualGameInput) -> Result<config::ManualGame, String> {
    let artwork_type = input.artwork_type.clone();
    let artwork_value = input.artwork_value.clone();

    let game = config::ManualGame {
        name: input.name,
        path: input.path,
        runner_type: if input.runner_type.is_empty() {
            "proton".into()
        } else {
            input.runner_type
        },
        proton_version: input.proton_version,
        use_global_args: input.use_global_args,
        arguments: input.arguments,
        dll_overrides: input.dll_overrides,
        ..Default::default()
    };
    let saved = config::add_manual_game(game);

    if artwork_type == "file" || artwork_type == "steam" {
        let (artwork, steam_id) = resolve_artwork(&artwork_type, artwork_value, &saved.id).await?;
        config::set_manual_artwork(&saved.id, artwork, steam_id);
    }

    let updated = config::load_games()
        .games
        .into_iter()
        .find(|g| g.id == saved.id)
        .unwrap_or(saved);
    Ok(updated)
}

#[tauri::command]
async fn update_manual_game(input: ManualGameInput) -> Result<(), String> {
    let id = input.id.clone();
    let artwork_type = input.artwork_type.clone();
    let artwork_value = input.artwork_value.clone();

    let game = config::ManualGame {
        id: id.clone(),
        name: input.name,
        path: input.path,
        runner_type: if input.runner_type.is_empty() {
            "proton".into()
        } else {
            input.runner_type
        },
        proton_version: input.proton_version,
        use_global_args: input.use_global_args,
        arguments: input.arguments,
        dll_overrides: input.dll_overrides,
        ..Default::default()
    };
    config::update_manual_game(game);

    if artwork_type == "file" || artwork_type == "steam" {
        let (artwork, steam_id) = resolve_artwork(&artwork_type, artwork_value, &id).await?;
        config::set_manual_artwork(&id, artwork, steam_id);
    } else if artwork_type == "none" {
        config::set_manual_artwork(&id, None, None);
    }
    refresh_manual_entry(&id);
    Ok(())
}

#[tauri::command]
fn delete_manual_game(id: String) {
    let _ = desktop_entry::remove("manual", &id);
    config::delete_manual_game(&id);
}

#[tauri::command]
fn set_desktop_entry(game: Game, enabled: bool) -> Result<bool, String> {
    if enabled {
        desktop_entry::create(&game)?;
    } else {
        desktop_entry::remove(&game.kind, &game.id)?;
    }
    Ok(enabled)
}

#[tauri::command]
async fn set_artwork(
    id: String,
    kind: String,
    artwork_type: String,
    artwork_value: Option<String>,
) -> Result<(), String> {
    let (artwork, steam_id) = if artwork_type == "none" {
        (None, None)
    } else {
        resolve_artwork(&artwork_type, artwork_value, &id).await?
    };
    match kind.as_str() {
        "manual" => config::set_manual_artwork(&id, artwork, steam_id),
        "steam" => config::steam_meta_set(&id, artwork, steam_id),
        "heroic" => config::heroic_meta_set(&id, artwork, steam_id),
        _ => return Err("Unknown game type".to_string()),
    }
    if kind == "manual" {
        refresh_manual_entry(&id);
    }
    Ok(())
}

#[tauri::command]
fn launch_game(app: AppHandle, state: State<'_, AppState>, game: Game) -> Result<(), String> {
    if state.running.lock().unwrap().contains_key(&game.id) {
        return Ok(());
    }
    match game.kind.as_str() {
        "manual" => {
            let settings = config::load_settings();
            let manual = config::load_games()
                .games
                .into_iter()
                .find(|g| g.id == game.id)
                .ok_or_else(|| "Game not found".to_string())?;
            let child = runner::spawn_manual(&manual, &settings)?;
            register_and_wait(
                app.clone(),
                child,
                game.id.clone(),
                game.kind.clone(),
                game.name.clone(),
            );
        }
        "steam" => {
            runner::spawn_uri(&format!("steam://rungameid/{}", game.id))?;
            track_latent(
                app.clone(),
                game.id.clone(),
                "steam".to_string(),
                game.name.clone(),
            );
        }
        "heroic" => {
            let command = game.command.clone().unwrap_or_else(|| {
                format!(
                    "heroic://launch/{}/{}",
                    game.platform.clone().unwrap_or_default(),
                    game.id
                )
            });
            runner::spawn_uri(&command)?;
            track_latent(
                app.clone(),
                game.id.clone(),
                "heroic".to_string(),
                game.name.clone(),
            );
        }
        _ => return Err("Unknown game type".to_string()),
    }
    Ok(())
}

#[tauri::command]
fn launch_native(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Ok(());
    }
    let mut game = config::load_games()
        .games
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| "Game not found".to_string())?;
    game.runner_type = "native".to_string();
    let settings = config::load_settings();
    let child = runner::spawn_manual(&game, &settings)?;
    register_and_wait(
        app,
        child,
        game.id.clone(),
        "manual".to_string(),
        game.name.clone(),
    );
    Ok(())
}

#[tauri::command]
fn stop_game(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let pid = {
        let running = state.running.lock().unwrap();
        running.get(&id).map(|entry| entry.pid)
    };
    if let Some(pid) = pid {
        let group_result = unsafe { libc::killpg(pid, libc::SIGTERM) };
        if group_result != 0 {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn running_games(state: State<'_, AppState>) -> Vec<String> {
    state.running.lock().unwrap().keys().cloned().collect()
}

#[tauri::command]
async fn protondb_tier(appid: String) -> Option<String> {
    protondb::get_tier(&appid).await
}

#[tauri::command]
async fn sgdb_cover(game_id: String) -> Option<String> {
    let settings = config::load_settings();
    let key = settings.get("sgdb_api_key").cloned().unwrap_or_default();
    artwork::sgdb_grid(&game_id, &key).await
}

#[tauri::command]
async fn search_steam_appid(name: String) -> Option<String> {
    artwork::search_steam_appid(&name).await
}

#[tauri::command]
fn open_external(target: String) -> Result<(), String> {
    runner::spawn_uri(&target)
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let expanded = if path.starts_with('~') {
        let rest = path.trim_start_matches('~').trim_start_matches('/');
        paths::home().join(rest).to_string_lossy().to_string()
    } else {
        path.clone()
    };
    let candidate = std::path::PathBuf::from(&expanded);
    let folder = if candidate.is_dir() {
        candidate.clone()
    } else {
        candidate.parent().map(|p| p.to_path_buf()).unwrap_or_default()
    };
    if folder.as_os_str().is_empty() || !folder.exists() {
        return Err("Folder does not exist".to_string());
    }
    let uri = format!("file://{}", folder.to_string_lossy());
    let opened = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowFolders",
        ])
        .arg(format!("array:string:{}", uri))
        .arg("string: ")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if opened {
        return Ok(());
    }
    runner::spawn_uri(&folder.to_string_lossy())
}

fn cli_launch_target() -> Option<(String, String)> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--launch-game" {
            return match (args.next(), args.next()) {
                (Some(kind), Some(id)) => Some((kind, id)),
                _ => None,
            };
        }
    }
    None
}

fn launch_game_from_cli(kind: &str, id: &str) -> Result<(), String> {
    match kind {
        "manual" => {
            let manual = config::load_games()
                .games
                .into_iter()
                .find(|game| game.id == id)
                .ok_or_else(|| "Game not found".to_string())?;
            let settings = config::load_settings();
            let mut child = runner::spawn_manual(&manual, &settings)?;
            let started = Instant::now();
            let _ = child.wait();
            config::add_playtime("manual", &manual.id, started.elapsed().as_secs() as i64);
        }
        "steam" => {
            runner::spawn_uri(&format!("steam://rungameid/{}", id))?;
        }
        "heroic" => {
            let command = heroic::scan()
                .into_iter()
                .find(|game| game.id == id)
                .and_then(|game| game.command)
                .unwrap_or_else(|| format!("heroic://launch/legendary/{}", id));
            runner::spawn_uri(&command)?;
        }
        _ => return Err(format!("Unknown game type: {}", kind)),
    }
    Ok(())
}

fn log_launch_error(error: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::cache_dir().join("launch.log"))
    {
        let _ = writeln!(file, "{}", error);
    }
}

pub fn run() {
    paths::ensure_dirs();
    if let Some((kind, id)) = cli_launch_target() {
        if let Err(error) = launch_game_from_cli(&kind, &id) {
            log_launch_error(&error);
            eprintln!("GameHub: {}", error);
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            scan_library,
            get_settings,
            save_settings,
            list_protons,
            proton_releases,
            install_proton,
            add_manual_game,
            update_manual_game,
            delete_manual_game,
            set_desktop_entry,
            set_artwork,
            launch_game,
            launch_native,
            stop_game,
            running_games,
            protondb_tier,
            sgdb_cover,
            search_steam_appid,
            open_external,
            open_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running GameHub");
}
