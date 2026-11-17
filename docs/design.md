# castanea launcher: design

The launcher keeps castanea at its newest release, starts it, and manages its mods. It is a native Rust program with no
web view. This page is the design; the other docs say how the pieces work in detail.

## Goals

- One window, one button: press Play, the game starts; see its output and a crash when there is one.
- One game install and one game folder. There is no version choice and no profiles: the launcher always runs the
  newest release, and updates itself to it at start.
- A mod manager (the installed mods) and a mod browser (an index of mods, install, update).
- Windows and Linux. Works offline with what is installed and cached.
- Safe with downloads: HTTPS only, SHA-256 checked, nothing downloaded is executed except the game.

Not goals: macOS, auto-update of the launcher itself, accounts, a mod upload flow, start options such as opening a
world or joining an address (the game has its own menu for that).

## GUI toolkit

| | egui / eframe | iced | Slint |
|---|---|---|---|
| Look and feel | plain, dense, easy to theme (dark default); "tool" look unless styled | polished widgets, Elm-style app | most polished, designer-style DSL |
| Windows | DX12, Vulkan or GL through wgpu, or GL through glow | wgpu or tiny-skia | femtovg, skia, software |
| Linux | winit: Wayland and X11, both tested widely | winit: Wayland and X11 | winit: Wayland and X11 |
| Text and IME | own text layout (epaint), IME through winit, CJK needs an extra font | cosmic-text, IME improving | good, IME through winit |
| Images | egui_extras loaders (PNG, JPEG) | built in | built in |
| Accessibility | AccessKit built in (screen readers on Windows and Linux) | none in the release | AccessKit |
| Binary size (release, stripped) | 23 MB measured here (wgpu, AccessKit, file dialogs); less with glow | about 10-15 MB | about 6-10 MB |
| Build time (clean) | fast, few heavy dependencies | slow (many crates) | slow (compiler for the `.slint` language) |
| Maturity | widely used, monthly releases, stable API style | pre-1.0, API changes each release | 1.x, stable, but GPLv3 or a royalty-free licence with attribution terms |
| Testing | `egui_kittest`, the UI runs headless in plain tests | little | `slint` testing backend |

**Choice: egui with eframe** (wgpu renderer). It is the most mature pure-Rust option on both platforms, has accessibility and
headless testing built in, builds fast, and needs no second language or licence decision. Immediate mode fits a launcher:
little state, much of it read from disk and from background jobs. The look is the launcher's own, from one set of tokens
(`ui/theme.rs`, [ui-design.md](ui-design.md)). The window uses the system frame on both platforms: snapping, shadows,
resizing and accessibility of the frame then work as the platform does them.

## Architecture

```
crates/core   castanea-launcher-core: no UI, no window, everything testable
  paths       the launcher's folders per platform (config, data, cache), overridable for tests and portable use
  settings    launcher.json: language, offline mode, index URLs, the game's start options, the local build folder
  migrate     moves an old home (profiles, side-by-side versions) to the one game folder and the one build, once
  install     the installed build, GameSource (GitHub releases), local build folders, the update (check, download, swap)
  launch      the launch plan (program, args, working dir, env) as pure data, the running game, its log and crash check
  mods        mod.json reading, the installed mods, enable/disable, import, dependency and version checks
  index       the ModIndex trait, the index format, the static-file and HTTP clients, the cache, updates
  download    HTTPS download with size limit, SHA-256 check, progress and atomic rename
crates/ui     castanea-launcher: the eframe window on top of core; strings from lang/*.json
```

The UI never touches the network on its frame: long work runs on a worker thread (`ui/jobs.rs`) and sends results back
over a channel. `ui/game.rs` holds the update state machine and the running game; `ui/app.rs` the rest of the state.

## Data on disk

| | Linux | Windows |
|---|---|---|
| config (`launcher.json`, `github-token`) | `$XDG_CONFIG_HOME/castanea-launcher` or `~/.config/castanea-launcher` | `%APPDATA%\castanea-launcher` |
| data (the build, the game folder, window state) | `$XDG_DATA_HOME/castanea-launcher` or `~/.local/share/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher` |
| cache (index, downloads, images) | `$XDG_CACHE_HOME/castanea-launcher` or `~/.cache/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher\cache` |

`CASTANEA_LAUNCHER_HOME=<dir>` puts all three below one folder (portable mode, tests, screenshots).

```
data/
  build/
    current/               the installed release: castanea[.exe], assets/, install.json (version, tag, asset, sha256)
    next/                  only during an update: the new build, checked and unpacked
    previous/              only during the swap: the old build, put back if the swap fails
  game/                    the game's working directory
    assets -> ../build/current/assets   a symlink (Linux) or junction (Windows), checked at each start
    mods/                  the mods; mods/.disabled/ holds disabled ones (the game skips names with a dot)
    saves/ settings.json crash/ debug/  written by the game
    logs/latest.log        the last run's output; logs/<stamp>.log for older runs (the last 10 kept)
  window/                  eframe's window state (size, position)
config/
  launcher.json github-token
cache/
  index/<sha256 of URL>.json and .meta   the last good copy of each index and its ETag
  downloads/<sha256>.zip                 a download in progress
```

The game reads `assets/` and `mods/` from its working directory (`docs/modding/modding.md` of the game), so the game
folder links the build's assets in. The link points at `build/current`, so it stays right across updates.

## The game build

- `GameSource` lists releases and installs one into a folder. `GithubReleases` reads `TerraNuxt/castanea` (the `repo`
  key of `launcher.json` can name another `owner/repo`).
- A release asset is a zip named `castanea-<version>-<target>.zip` with the binary and `assets/` at the top (or in one
  top folder). Its SHA-256 comes from GitHub's asset `digest` field, or from a `SHA256SUMS` asset of the release. A build
  without either is refused.
- **Newest**: the release with the highest version (semver) that has an archive for this platform; drafts are skipped,
  pre-releases count only when there is no other release.
- **At start** the launcher checks the source once (not in offline mode, not with a local build folder). A release
  that is another tag and not older than the installed one is downloaded with progress, checked against its SHA-256,
  and unpacked into `build/next`. The installed build is untouched until then, and the game can still be played.
- **The swap** (`update::activate`): `current` moves to `previous`, `next` becomes `current`, `previous` is deleted.
  If moving `next` fails, `previous` is put back. If the launcher stops half way, `previous` is found and used at the
  next start. The swap waits while the game runs (Windows cannot move a folder with a running program in it); it
  happens when the game ends.
- **A failed download** (network, size, SHA-256 mismatch, a zip without the game) deletes what it wrote; the installed
  build stays and the home page says the check failed, with Try again. Old builds are never kept or offered as a choice.
- **Offline** (offline mode, or no network): no check; the installed build is used. With nothing installed, Play is
  off and the home page says to go online once.
- **A local build folder** (Settings → Advanced, `--game-dir DIR` or `CASTANEA_GAME_DIR` for one run) is for people who
  build the game from source: a checkout (`target/release/castanea` and `assets/`) or a build folder is run in place,
  never copied or changed, and there are no updates while it is set.
- A private game repo needs a token: `CASTANEA_GITHUB_TOKEN` or `GITHUB_TOKEN`, or the file `config/github-token`
  (Settings → Advanced, mode 0600 on Linux). The token is sent only to `api.github.com` and GitHub's download hosts,
  never logged.
- The version is the release tag, or a `version.txt` next to the binary of a local build, or `local`.

## Old homes

`migrate::run` runs at each start and does nothing on a new home. A home of the launcher with profiles is moved once:

- The options of the last used profile (player name, relay `backend_url`, graphics backend, window size, extra
  arguments, environment) become the global ones in `launcher.json`. Its start option (a world, joining an address) is
  dropped: the game has its own menu.
- Its game folder `data/instances/<id>` becomes `data/game` (saves, settings and mods move with it). The folders of
  other profiles stay where they are; the launcher says where, once.
- The newest downloaded build in `data/versions` becomes `data/build/current`; the other downloaded builds are deleted.
  Folders the user had added were registered in place and are not touched.
- `profiles.json` is renamed `profiles.json.old`; `installs.json` is deleted; unknown keys of `launcher.json`
  (`sources`, `last_profile`, `system_frame`) are dropped at the next save.

## Launching

- The plan is pure data built by `launch::plan` and tested: program = the build's binary, working dir = the game
  folder, args = `[--name N] <extra args>`, env = `CASTANEA_BACKEND` (vulkan or wgpu, or the game's own setting),
  `CASTANEA_WINDOW`, and the extra variables.
- `backend_url` is written into the game folder's `settings.json` (the game keeps unknown keys) before the start.
- Output (stdout and stderr) goes to `logs/latest.log` and to the Game output page. On Windows the game starts without
  a console window. A crash is an exit code other than 0 or a new file in `crash/`; the launcher shows the report.
- The game runs once at a time; while it runs, Play is Stop and shows the running time.

## Mods

- A mod is a folder or a zip with `mod.json` at the top (`id`, `name`, `version`, `author`, `depends`, `wasm`, `side`,
  `aliases`), the game's own format. The launcher reads it tolerantly (it shows broken packages with their error).
- Enabled mods are in `mods/`, disabled ones in `mods/.disabled/`. Moving is atomic on one disk.
- Checks: missing or disabled dependency, two mods with one id, a dependency cycle, and the game version range from the
  index (mod.json has no game version key; the game refuses unknown keys, so the range lives in the index).
- Import from a zip or folder: the archive is checked (mod.json at the top, no absolute or `..` paths, size limits) and
  lands disabled.
- Per-world switches (`world.mods` in a save) stay the game's business.

## Mod browser and index

`docs/mod-index.md` specifies `index.json` and the HTTP API the backend may offer later. `ModIndex` has a static-file
implementation (a local path or an `https://` URL to one JSON file) and caches the last good copy, so the browser works
offline. Install: download, check size and SHA-256 from the index, check the zip, put it into the mods folder disabled,
then the user enables it. Updates: an installed mod whose id is in the index with a newer compatible version.

## Security model

- Downloads use HTTPS only. Plain `http://` is refused, except to `127.0.0.1` and `localhost` when a test asks for it.
- Every download has a SHA-256 from a trusted place (the index, GitHub's digest or `SHA256SUMS`) and a size limit; a
  mismatch deletes the file and keeps the installed build. Signatures (Ed25519 over the index, field `signature`) are
  planned: the format has room for them, the client does not check them yet.
- The launcher never runs downloaded code except the game binary, and it never runs anything from a mod. Mods are
  untrusted data: archives are unpacked by the game, not by the launcher, but are checked for unsafe paths on import.
  Wasm in a mod runs only inside the game's sandbox; native code cannot come with a mod (Rust `ModApi` mods are
  compiled into the game).
- A new mod lands disabled. Enabling shows what it declares: its side, whether it carries Wasm code, its dependencies,
  aliases and file count.
- Tokens are read from the environment or a private file, never stored in `launcher.json` or the log.

## Open decisions for the user

- Release asset naming and contents (proposed above), and whether the game should print `--version` or ship `version.txt`.
- Whether `mod.json` should get a `game` key (a version range) so the range does not depend on the index.
- Where the public index lives (a file in a repo, the backend, a CDN) and who signs it.
- Whether the launcher should update itself.
