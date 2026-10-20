# GameHub

A unified game launcher for Linux. It brings your Steam, Heroic (Epic / GOG) and manual
Windows games into one clean library, with Proton management, ProtonDB ratings and
SteamGridDB artwork.

## Features

- **Unified library** — automatically scans Steam and Heroic (Epic, GOG) titles.
- **Manual games** — add any Windows or native executable with a custom configuration.
- **ProtonDB integration** — compatibility ratings (Platinum, Gold, Silver, Bronze, Borked)
  shown right on the game cards.
- **SteamGridDB artwork** — high-quality covers, plus local file covers and automatic
  icon extraction from `.exe` files (via `icoutils`).
- **Proton manager** — download and install GE-Proton, Proton-CachyOS, Proton-EM and
  Proton-Sarek releases from GitHub without leaving the app (checksum-verified).
- **Runners** — Proton (global default, per-game version or native execution), custom
  launch arguments, per-game or global, and `WINEDLLOVERRIDES` support.
- **Playtime tracking** — sessions are timed and playtime is accumulated per game. For
  Steam games, local playtime is merged with your Steam profile playtime.
- **Steam Web API** — optionally list your uninstalled Steam games (with download links).

## Prerequisites

- Linux with a desktop environment (X11 or Wayland)
- Steam installed (for launching Steam titles) and/or Heroic
- The `icoutils` package (`wrestool`, `icotool`) for `.exe` icon extraction
- `xdg-open` for launching URIs (`steam://`, `heroic://`)

## Installation

Download the standalone `gamehub` binary or the `.AppImage` from the
[Releases](https://github.com/ViaFaniq/GameHub/releases) page:

```bash
chmod +x GameHub_<version>_amd64.AppImage
./GameHub_<version>_amd64.AppImage
```

## Building from source

Requirements: Rust (stable), `cargo-tauri` CLI v2.

```bash
cd src-tauri
cargo tauri build --bundles appimage
```

Artifacts:

- `src-tauri/target/release/gamehub`
- `src-tauri/target/release/bundle/appimage/GameHub_<version>_amd64.AppImage`

## Configuration

All app data lives in familiar locations:

- `~/.config/gamehub/` — settings (`configuration.conf`), manual games (`games.json`),
  artwork overrides and playtime caches
- `~/.cache/gamehub/` — artwork cache, ProtonDB cache, Steam app list cache
- `~/.local/share/gamehub/` — installed Proton versions and per-game Wine prefixes

## Notes

- The app expects `steam` and `awww`-independent external launchers: Steam titles are
  started through `steam://rungameid/<appid>`, Heroic titles through
  `heroic://launch/<platform>/<id>`.
- Playtime for Steam games is synced with the Steam Web API and always keeps the highest
  value seen.

## License

GPL-3.0 — see [LICENSE](LICENSE).
