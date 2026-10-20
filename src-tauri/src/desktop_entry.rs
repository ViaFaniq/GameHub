use std::path::{Path, PathBuf};

use crate::model::Game;
use crate::paths;

const APP_ICON: &[u8] = include_bytes!("../icons/128x128.png");

fn applications_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| paths::home().join(".local/share"))
        .join("applications")
}

fn icons_dir() -> PathBuf {
    paths::data_dir().join("icons")
}

fn sanitize(value: &str) -> String {
    let mut out: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    out
}

fn entry_path(kind: &str, id: &str) -> PathBuf {
    applications_dir().join(format!(
        "gamehub-{}-{}.desktop",
        sanitize(kind),
        sanitize(id)
    ))
}

pub fn exists(kind: &str, id: &str) -> bool {
    entry_path(kind, id).is_file()
}

fn launcher_executable() -> PathBuf {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        let path = PathBuf::from(&appimage);
        if path.is_file() {
            return path;
        }
    }
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gamehub"))
}

fn app_icon() -> Option<PathBuf> {
    let directory = icons_dir();
    std::fs::create_dir_all(&directory).ok()?;
    let target = directory.join("gamehub.png");
    if !target.is_file() {
        std::fs::write(&target, APP_ICON).ok()?;
    }
    Some(target)
}

fn game_icon(game: &Game) -> Option<PathBuf> {
    let Some(artwork) = game
        .artwork
        .as_deref()
        .filter(|value| !value.starts_with("http://") && !value.starts_with("https://"))
    else {
        return app_icon();
    };
    let source = Path::new(artwork);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(extension.as_str(), "png" | "svg" | "xpm") || !source.is_file() {
        return app_icon();
    }
    let directory = icons_dir();
    if std::fs::create_dir_all(&directory).is_err() {
        return app_icon();
    }
    let target = directory.join(format!(
        "game-{}-{}.{}",
        sanitize(&game.kind),
        sanitize(&game.id),
        extension
    ));
    if source != target && std::fs::copy(source, &target).is_err() {
        return app_icon();
    }
    Some(target)
}

fn escape_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ")
}

fn quote_exec(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for ch in value.chars() {
        match ch {
            '\\' | '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn render(game: &Game, executable: &Path, icon: Option<&Path>) -> String {
    let mut entry = String::new();
    entry.push_str("[Desktop Entry]\n");
    entry.push_str("Type=Application\n");
    entry.push_str("Version=1.0\n");
    entry.push_str(&format!("Name={}\n", escape_value(&game.name)));
    entry.push_str(&format!(
        "Comment=Play {} with GameHub\n",
        escape_value(&game.name)
    ));
    entry.push_str(&format!(
        "Exec={} --launch-game {} {}\n",
        quote_exec(&executable.to_string_lossy()),
        quote_exec(&game.kind),
        quote_exec(&game.id)
    ));
    if let Some(icon) = icon {
        entry.push_str(&format!("Icon={}\n", icon.to_string_lossy()));
    }
    entry.push_str("Terminal=false\n");
    entry.push_str("Categories=Game;\n");
    entry.push_str("StartupNotify=false\n");
    entry.push_str(&format!("X-GameHub-Game={}:{}\n", game.kind, game.id));
    entry
}

fn write_entry(
    directory: &Path,
    game: &Game,
    executable: &Path,
    icon: Option<&Path>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create applications folder: {error}"))?;
    let target = directory.join(format!(
        "gamehub-{}-{}.desktop",
        sanitize(&game.kind),
        sanitize(&game.id)
    ));
    std::fs::write(&target, render(game, executable, icon))
        .map_err(|error| format!("Could not write menu entry: {error}"))?;
    Ok(target)
}

pub fn create(game: &Game) -> Result<PathBuf, String> {
    if !matches!(game.kind.as_str(), "steam" | "heroic" | "manual") {
        return Err("Unknown game type".to_string());
    }
    if game.name.trim().is_empty() {
        return Err("Game name is empty".to_string());
    }
    let executable = launcher_executable();
    if !executable.is_file() {
        return Err(format!(
            "GameHub executable not found: {}",
            executable.display()
        ));
    }
    let icon = game_icon(game);
    write_entry(&applications_dir(), game, &executable, icon.as_deref())
}

pub fn remove(kind: &str, id: &str) -> Result<(), String> {
    let target = entry_path(kind, id);
    match std::fs::remove_file(&target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove menu entry: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_game() -> Game {
        Game {
            id: "12345".into(),
            name: "Test Game".into(),
            kind: "manual".into(),
            installed: true,
            ..Default::default()
        }
    }

    #[test]
    fn renders_desktop_entry() {
        let entry = render(
            &sample_game(),
            Path::new("/opt/gamehub/gamehub"),
            Some(Path::new(
                "/home/user/.local/share/gamehub/icons/gamehub.png",
            )),
        );
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("Type=Application\n"));
        assert!(entry.contains("Name=Test Game\n"));
        assert!(
            entry.contains("Exec=\"/opt/gamehub/gamehub\" --launch-game \"manual\" \"12345\"\n")
        );
        assert!(entry.contains("Icon=/home/user/.local/share/gamehub/icons/gamehub.png\n"));
        assert!(entry.contains("Terminal=false\n"));
        assert!(entry.contains("Categories=Game;\n"));
    }

    #[test]
    fn quotes_special_characters_in_exec() {
        let quoted = quote_exec("/tmp/A B/$game`test`%/x");
        assert_eq!(quoted, "\"/tmp/A B/\\$game\\`test\\`%%/x\"");
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize("a b/c:d"), "a_b_c_d");
        assert_eq!(sanitize(""), "_");
    }

    #[test]
    fn writes_entry_in_target_directory() {
        let directory =
            std::env::temp_dir().join(format!("gamehub-desktop-entry-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let path = write_entry(
            &directory,
            &sample_game(),
            Path::new("/opt/gamehub/gamehub"),
            None,
        )
        .expect("entry should be written");
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "gamehub-manual-12345.desktop"
        );
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Name=Test Game"));

        if let Ok(output) = std::process::Command::new("desktop-file-validate")
            .arg(&path)
            .output()
        {
            if output.status.code().is_some() {
                assert!(
                    output.status.success(),
                    "desktop-file-validate failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    #[ignore]
    fn creates_and_removes_real_entry() {
        let game = Game {
            id: "gamehub-test-entry".into(),
            name: "GameHub Test Entry".into(),
            kind: "steam".into(),
            installed: true,
            ..Default::default()
        };
        let path = create(&game).expect("entry should be created");
        assert!(path.is_file());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("--launch-game \"steam\" \"gamehub-test-entry\""));
        if let Ok(output) = std::process::Command::new("desktop-file-validate")
            .arg(&path)
            .output()
        {
            if output.status.code().is_some() {
                assert!(
                    output.status.success(),
                    "desktop-file-validate failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        remove("steam", "gamehub-test-entry").expect("entry should be removed");
        assert!(!path.exists());
    }
}
