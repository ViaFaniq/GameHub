use std::path::PathBuf;

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn config_dir() -> PathBuf {
    home().join(".config/gamehub")
}

pub fn cache_dir() -> PathBuf {
    home().join(".cache/gamehub")
}

pub fn art_cache_dir() -> PathBuf {
    cache_dir().join("artworks")
}

pub fn data_dir() -> PathBuf {
    home().join(".local/share/gamehub")
}

pub fn proton_install_dir() -> PathBuf {
    data_dir().join("proton")
}

pub fn prefix_dir(id: &str) -> PathBuf {
    data_dir().join("prefixes").join(id)
}

pub fn steam_roots() -> Vec<PathBuf> {
    vec![
        home().join(".steam/steam"),
        home().join(".local/share/Steam"),
        home().join(".var/app/com.valvesoftware.Steam/.steam/steam"),
    ]
}

pub fn heroic_roots() -> Vec<PathBuf> {
    vec![
        home().join(".config/heroic"),
        home().join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ]
}

pub fn ensure_dirs() {
    for dir in [
        config_dir(),
        cache_dir(),
        art_cache_dir(),
        data_dir(),
        proton_install_dir(),
    ] {
        let _ = std::fs::create_dir_all(dir);
    }
}
