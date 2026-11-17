# castanea launcher

A native launcher for [castanea](https://github.com/TerraNuxt/castanea): it starts the game, keeps several game
versions side by side, and is a mod manager and mod browser. Rust with [egui](https://github.com/emilk/egui), no web
view. Windows and Linux.

![Play tab](docs/screenshots/play.png)

| | |
|---|---|
| ![Mods, installed](docs/screenshots/mods-installed.png) | ![Mods, browse](docs/screenshots/mods-browse.png) |
| ![Enable a mod](docs/screenshots/mods-enable.png) | ![Game log](docs/screenshots/play-log.png) |
| ![Profile options](docs/screenshots/profile.png) | ![German](docs/screenshots/mods-installed-de.png) |

## What it does

- **Play**: pick a profile and a game version, press Play (F5). The game's output shows live and goes to
  `logs/latest.log` in the profile folder; a crash (exit code or a new report in `crash/`) is shown with its report.
- **Profiles**: each has its own game folder (saves, settings, mods), and options: start in the menu, a world (seed,
  host port) or join a server; player name; `backend_url`; graphics backend (`CASTANEA_BACKEND=vulkan|wgpu`); window
  size; extra arguments and environment variables.
- **Versions**: builds found on disk, folders you add (a release folder or a checkout with `target/release`), and
  releases of `TerraNuxt/castanea` (or any GitHub repo) downloaded with a SHA-256 check. A private repo needs a token.
- **Mods → Installed**: the profile's mods from `mod.json`: enable, disable, remove, import a zip or folder, open the
  folder; missing or disabled dependencies, duplicate ids, cycles and game-version mismatches are shown.
- **Mods → Browse**: search and filter an index ([format](docs/mod-index.md)), details, install with progress and hash
  check, updates. Works offline from the cache.
- English and German (`crates/ui/lang/*.json`), dark theme, keyboard shortcuts (Ctrl+1…5, F5, Ctrl+F, Tab), window
  size and position remembered.

Design and security model: [docs/design.md](docs/design.md). Packaging: [docs/packaging.md](docs/packaging.md).
Testing on Windows: [docs/windows.md](docs/windows.md).

## Build

```sh
cargo run --release -p castanea-launcher
cargo test --workspace
```

Rust 1.92 or newer. On Linux the window needs the usual runtime libraries (Vulkan or GL driver, X11 or Wayland,
xkbcommon); `tools/shell.nix` has them for NixOS. On Windows, the MSVC toolchain.

`CASTANEA_LAUNCHER_HOME=<dir>` keeps everything below one folder (portable mode). `CASTANEA_GAME_DIR=<dir>` points the
launcher at a game folder. `CASTANEA_GITHUB_TOKEN` gives the token for a private game repo.

## Layout

| Path | What |
|---|---|
| `crates/core` | Everything without UI: folders, settings, builds and sources, profiles, launch, mods, index, downloads |
| `crates/ui` | The window (eframe): `app.rs` state and actions, `shell.rs` frame, `tabs/` one file per tab, `lang/` strings |
| `docs/` | Design, mod index format, packaging, Windows notes, screenshots, an example index |
| `tools/remote.sh` | Runs a command on the build host (`tools/remote.sh cargo test --workspace`) |
| `tools/winck.sh` | Windows cross-check (cargo check and clippy for a Windows target in a container) |
| `tools/screenshots.sh` | Takes `docs/screenshots` under a private Xvfb display (`castanea-launcher --screenshots DIR`) |

## License

MIT, see [LICENSE](LICENSE).
