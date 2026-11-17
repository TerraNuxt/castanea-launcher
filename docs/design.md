# castanea launcher: design

The launcher starts castanea and manages its builds, profiles and mods. It is a native Rust program with no web view.
This page is the design to review; the other docs say how the pieces work in detail.

## Goals

- One window that starts the game: pick a profile and a game version, press Play, see the log, see a crash.
- Several game versions side by side, each profile with its own game folder (saves, settings, mods).
- A mod manager (installed mods of a profile) and a mod browser (an index of mods, install, update).
- Windows and Linux from the first commit. Works offline with what is installed and cached.
- Safe with downloads: HTTPS only, SHA-256 checked, nothing downloaded is executed except the game binary the user chose.

Not goals for now: macOS, auto-update of the launcher itself, accounts, a mod upload flow.

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
little state, much of it read from disk and from background jobs. The look is made our own with a theme (`ui/theme.rs`).
wgpu picks DX12 or Vulkan on Windows and Vulkan or GL on Linux, so a PC without a good OpenGL driver still works; a `glow`
cargo feature gives the small OpenGL build if size matters later.

## Architecture

```
crates/core   castanea-launcher-core: no UI, no window, everything testable
  paths       the launcher's folders per platform (config, data, cache), overridable for tests and portable use
  settings    launcher.json: language, sources, index URLs, offline mode
  install     the install registry (versions side by side), GameSource trait, GitHub releases, local folder, archives
  profile     profiles (instances): game version, folder, world, name, backend_url, extra args, env, graphics backend
  launch      the launch plan (program, args, working dir, env) as pure data, the running game, its log and crash check
  mods        mod.json reading, the installed mods of a profile, enable/disable, import, dependency and version checks
  index       the ModIndex trait, the index format, the static-file and HTTP clients, the cache, updates
  download    HTTPS download with size limit, SHA-256 check, progress and atomic rename
crates/ui     castanea-launcher: the eframe window on top of core; strings from lang/*.json
```

The UI never touches the disk or the network on its frame: long work runs on a worker thread (`ui/jobs.rs`) and sends
results back over a channel. A command line front end later is a second small crate on `core`.

## Data on disk

| | Linux | Windows |
|---|---|---|
| config (`launcher.json`, `profiles.json`, `github-token`) | `$XDG_CONFIG_HOME/castanea-launcher` or `~/.config/castanea-launcher` | `%APPDATA%\castanea-launcher` |
| data (versions, instances, window state) | `$XDG_DATA_HOME/castanea-launcher` or `~/.local/share/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher` |
| cache (index, downloads, images) | `$XDG_CACHE_HOME/castanea-launcher` or `~/.cache/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher\cache` |

`CASTANEA_LAUNCHER_HOME=<dir>` puts all three below one folder (portable mode, tests, screenshots).

```
data/
  versions/<id>/           a managed game build: castanea[.exe], assets/, install.json (source, version, sha256)
  instances/<profile>/     the game's working directory for one profile
    assets -> versions/<id>/assets      a symlink (Linux) or junction (Windows), set at each launch
    mods/                  the profile's mods; mods/.disabled/ holds disabled ones (the game skips names with a dot)
    saves/ settings.json crash/ debug/  written by the game
    logs/latest.log        the last run's output; logs/<stamp>.log for older runs (the last 10 kept)
  window/                  eframe's window state (size, position)
config/
  launcher.json profiles.json installs.json
cache/
  index/<sha256 of URL>.json and .meta   the last good copy of each index and its ETag
  downloads/<sha256>.part               downloads in progress
```

The game reads `assets/` and `mods/` from its working directory (`docs/modding/modding.md` of the game). So each profile
gets its own folder, and the build's assets are linked in, not copied. A build added from a local folder (a checkout with
`target/release/castanea` and `assets/`) is registered in place, never copied or changed.

## Game versions

- `GameSource` lists builds and fetches one into `data/versions/<id>`. Implementations: `GithubReleases`
  (`TerraNuxt/castanea` by default, any `owner/repo`), `LocalFolder` (registers a folder in place).
- A release asset is a zip named `castanea-<version>-<target>.zip` with the binary and `assets/` at the top (or in one
  top folder). Its SHA-256 comes from GitHub's asset `digest` field, or from a `SHA256SUMS` asset of the release. A build
  without either is refused.
- A private repo needs a token: `CASTANEA_GITHUB_TOKEN` or `GITHUB_TOKEN`, or the file `config/github-token` (written by the
  Settings tab, mode 0600 on Linux). The token is sent only to `api.github.com` and GitHub's download hosts, never logged.
- The game has no `--version` yet; the version is the release tag, or a `version.txt` next to the binary, or `local`.

## Launching

- The plan is pure data built by `launch::plan` and tested: program = the build's binary, working dir = the instance,
  args = `[seed] [--world W] [--name N] [--join A | --host P] <extra args>`, env = `CASTANEA_BACKEND` (auto, vulkan or
  wgpu), `CASTANEA_WINDOW`, and the profile's extra variables.
- `backend_url` is written into the instance's `settings.json` (the game keeps unknown keys) before the start.
- Output (stdout and stderr) goes to `logs/latest.log` and to the Play tab. On Windows the game starts without a
  console window. A crash is an exit code other than 0 or a new file in `crash/`; the launcher shows the report.
- Memory: the game has no heap setting (no JVM); the profile offers the graphics backend and the window size instead.

## Mods

- A mod is a folder or a zip with `mod.json` at the top (`id`, `name`, `version`, `author`, `depends`, `wasm`, `side`,
  `aliases`), the game's own format. The launcher reads it tolerantly (it shows broken packages with their error).
- Enabled mods are in `mods/`, disabled ones in `mods/.disabled/`. Moving is atomic on one disk.
- Checks: missing or disabled dependency, two mods with one id, a dependency cycle, and the game version range from the
  index (mod.json has no game version key; the game refuses unknown keys, so the range lives in the index).
- Import from a zip or folder: the archive is checked (mod.json at the top, no absolute or `..` paths, size limits) and
  lands disabled.
- Per-world switches (`world.mods` in a save) stay the game's business; the launcher works on the profile.

## Mod browser and index

`docs/mod-index.md` specifies `index.json` and the HTTP API the backend may offer later. `ModIndex` has a static-file
implementation (a local path or an `https://` URL to one JSON file) and caches the last good copy, so the browser works
offline. Install: download, check size and SHA-256 from the index, check the zip, put it into the profile's mods
folder disabled, then the user enables it. Updates: an installed mod whose id is in the index with a newer compatible
version.

## Security model

- Downloads use HTTPS only. Plain `http://` is refused, except to `127.0.0.1` and `localhost` when a test asks for it.
- Every download has a SHA-256 from a trusted place (the index, GitHub's digest or `SHA256SUMS`) and a size limit; a
  mismatch deletes the file. Signatures (Ed25519 over the index, field `signature`) are planned: the format has room for
  them, the client does not check them yet.
- The launcher never runs downloaded code except the game binary of the build the user picked, and it never runs
  anything from a mod. Mods are untrusted data: archives are unpacked by the game, not by the launcher, but are checked
  for unsafe paths on import. Wasm in a mod runs only inside the game's sandbox; native code cannot come with a mod
  (Rust `ModApi` mods are compiled into the game).
- A new mod lands disabled. Enabling shows what it declares: its side, whether it carries Wasm code, its dependencies,
  aliases and file count.
- Tokens are read from the environment or a private file, never stored in `launcher.json` or the log.

## Open decisions for the user

- Release asset naming and contents (proposed above), and whether the game should print `--version` or ship `version.txt`.
- Whether `mod.json` should get a `game` key (a version range) so the range does not depend on the index.
- Where the public index lives (a file in a repo, the backend, a CDN) and who signs it.
- Whether the launcher should update itself.
