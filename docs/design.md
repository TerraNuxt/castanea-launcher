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

**Fonts.** egui draws TrueType text only, so the project's own bitmap font (BMFont atlas, coloured, capitals and digits
only) is loaded by a small parser in `ui/bmfont.rs`, scaled on the CPU into texture levels and drawn as textured quads
(`ui/bmfont/`). It sets the wordmark, the page titles and the Play label; Inter stays the text face and the fallback for
anything the atlas cannot spell. Details and the reasons are in [ui-design.md](ui-design.md#the-brand-font).

## Architecture

```
crates/core   castanea-launcher-core: no UI, no window, everything testable
  paths       the launcher's folders per platform (config, data, cache), overridable for tests and portable use
  settings    launcher.json: language, offline mode, index URLs, release server and beta switch, the game's start options, the local build folder
  playername  the player name: typed name > OS name > none, tidying, and the 3-16 character form the game accepts (player-name.md)
  osname      the OS account's display name per platform (GetUserNameExW, getpwuid_r GECOS) and its pure parsers
  gallery     the game's screenshots: scan and sort, day groups, thumbnails and their cache, rename, delete, trash (screenshots-gallery.md)
  migrate     moves an old home (profiles, side-by-side versions) to the one game folder and the one build, once
  install     the installed build, GameSource and the release API client, local build folders, the update (check, download, swap)
  launch      the launch plan (program, args, working dir, env) as pure data, the running game, its log and crash check
  backend     the graphics backend choice: the game's GPU probe, its cache per game version and GPU, the Vulkan crash memory (game-launch.md)
  mods        mod.json reading, the installed mods, enable/disable, import, dependency and version checks
  index       the ModIndex trait, the index format, the static-file and HTTP clients, the cache, updates
  download    HTTPS download with size limit, SHA-256 check, progress and speed, retry with backoff, resumable (Range) downloads
  signature   Ed25519 release signatures: the canonical message and the check; trust: the embedded public keys
  selfupdate  the launcher replacing itself: check, stage, apply plans (Linux rename, Windows rename trick), install location, rollback state
crates/ui     castanea-launcher: the eframe window on top of core; strings from lang/*.json
```

The UI never touches the network on its frame: long work runs on a worker thread (`ui/jobs.rs`) and sends results back
over a channel. `ui/game.rs` holds the game's update state machine and the running game, `ui/launcher.rs` the launcher's own; `ui/app.rs` the rest of the state.

## Data on disk

| | Linux | Windows |
|---|---|---|
| config (`launcher.json`) | `$XDG_CONFIG_HOME/castanea-launcher` or `~/.config/castanea-launcher` | `%APPDATA%\castanea-launcher` |
| data (the build, the game folder, window state) | `$XDG_DATA_HOME/castanea-launcher` or `~/.local/share/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher` |
| cache (index, downloads, images, news) | `$XDG_CACHE_HOME/castanea-launcher` or `~/.cache/castanea-launcher` | `%LOCALAPPDATA%\castanea-launcher\cache` |

`CASTANEA_LAUNCHER_HOME=<dir>` puts all three below one folder (portable mode, tests, screenshots).

```
data/
  build/
    current/               the installed release: castanea[.exe], assets/, install.json (version, channel, released_at, notes, sha256)
    next/                  only during an update: the new build, checked and unpacked (a leftover from a stopped update is removed at start)
    previous/              only during the swap: the old build, deleted once the new one is in place (a leftover is removed at start)
  game/                    the game's working directory
    assets -> ../build/current/assets   a symlink (Linux) or junction (Windows), checked at each start
    mods/                  the mods; mods/.disabled/ holds disabled ones (the game skips names with a dot)
    saves/ screenshots/ settings.json crash/ debug/  written by the game
    running.json           the pid of the running game, for a launcher that starts later
    logs/latest.log        the last run's output; logs/<stamp>.log for older runs (the last 10 kept)
  backend.json             what the launcher learned about the graphics backend: probe results, Vulkan starts that died, the last start
  window/                  eframe's window state (size, position)
  launcher-update/         the launcher's own update: staged/ (unpacked new launcher), staged.json, update-pending (the rollback marker), skip.json (versions that failed to start)
config/
  launcher.json
cache/
  index/<sha256 of URL>.json and .meta   the last good copy of each index and its ETag
  downloads/<sha256>.part                a download in progress, kept to be resumed
  news/<lang>.json                       the last good news feed per language and its ETag (docs/release-api.md, "News")
  thumbnails/<key>.jpg                   the gallery's thumbnails, pruned to the files that exist and to 96 MB
```

The game reads `assets/` and `mods/` from its working directory (`docs/modding/modding.md` of the game), so the game
folder links the build's assets in. The link points at `build/current`, so it stays right across updates.

## The game build

- `GameSource` offers the newest release and installs one into a folder; `ReleaseApi` is the one real implementation. The
  private game repo is no longer read: builds come from the release server (`https://castanea.xenoo.dev`, the
  `release_url` key of `launcher.json`, or `CASTANEA_RELEASE_URL`). The contract, the error handling and the proposed
  changes are in [release-api.md](release-api.md).
- The launcher asks for its own platform (`linux-x86_64` or `windows-x86_64`, from the OS and processor it runs on; on
  another system the home page says there is no build yet) and the channel `stable`, plus `beta` when Settings →
  Advanced → Include beta builds is on. The newer version of the two wins.
- A release is a zip with `castanea` (`castanea.exe`) and `assets/` at its root, plus optionally `version.txt`. The API
  gives its size and SHA-256; a zip without the executable or `assets/` is refused.
- **Newest**: the release the server calls latest. It is installed when its version differs from the installed one and
  is not older (never a downgrade; the same version is a no-op).
- **At start** the launcher asks once (not in offline mode, not with a local build folder). A newer release is downloaded
  with progress (bytes, percent and speed), checked against its SHA-256, and unpacked into `build/next`. The installed
  build is untouched until then, and the game can still be played. "What's new" on the home page shows the release
  notes (markdown reduced to plain text); they are kept in `install.json`, so they are there offline too.
- **Resumable download**: the archive is written to `cache/downloads/<sha256>.part`. After a network drop the launcher
  waits (backoff, `Retry-After`) and continues with `Range: bytes=<have>-` and `If-Range: "<sha256>"`; the same happens
  after a restart, because the partial file stays. The server's `ETag` must be the SHA-256 of the release, so the
  bytes of a resumed file are from one build; a server that ignores `Range` (answers 200) restarts the file. The
  finished file is hashed as a whole; a mismatch deletes it. Partial files of other builds are deleted when a new
  download starts.
- **The swap** (`update::activate`): `current` moves to `previous`, `next` becomes `current`, `previous` is deleted, and
  the archive of the installed build is removed. Nothing else is kept: no old build, no copy, no partial file of it. If
  moving `next` fails, `previous` is put back and the old build stays as it was. A folder a program or a virus scanner
  still holds is retried quietly for a second and then left; the start-up clean-up removes it later, and a failure of
  the removal never fails the update. The swap waits while the game runs (Windows cannot move a folder with a running
  program in it); it happens when the game ends.
- **Start-up clean-up** (`update::clean_leftovers`, before the window opens and before any download): `next` is removed;
  if `current` is whole, `previous` is removed; if `current` is gone or broken and `previous` is a whole build (a swap that
  stopped half way), `previous` is put back as `current`.
- **A failed download** (network, size, SHA-256 mismatch, a zip without the game) deletes what it wrote, except a
  partial file that can be resumed; a zip that unpacks but fails its check takes its `next` folder with it; the installed build stays and the home page says the check failed, with Try again.
  Old builds are never kept or offered as a choice.
- **Offline** (offline mode, or no network): no check; the installed build is used. With nothing installed, Play is
  off and the home page says to go online once.
- **A local build folder** (Settings → Advanced, `--game-dir DIR` or `CASTANEA_GAME_DIR` for one run) is for people who
  build the game from source: a checkout (`target/release/castanea` and `assets/`) or a build folder is run in place,
  never copied or changed, and there are no updates while it is set.
- Plain `http://` is accepted for the release server only on the loopback (tests). No token is needed or stored; the
  `github-token` file of older launchers is deleted at start.
- The version is the release's `version`, or a `version.txt` next to the binary of a local build, or `local`.

## Old homes

`migrate::run` runs at each start and does nothing on a new home. A home of the launcher with profiles is moved once:

- The options of the last used profile (player name, graphics backend, window size, extra
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
  folder, args = `--menu [--name N] <extra args>` (N: the typed name or the OS account's, in the game's form,
  [player-name.md](player-name.md)), env = `CASTANEA_BACKEND` (vulkan or wgpu, left out when the game decides),
  `CASTANEA_WINDOW`, and the extra variables.
- **Which backend** is decided before the start by `backend::choose`, on a worker thread: Vulkan first, but only when a probe of
  the installed game (`--probe-gpu`, cached per game version and GPU) says it works, else wgpu, else an error; a game without
  the probe starts with its own `auto`; a Vulkan start that dies within seconds is remembered. Rules and the game-side contract:
  [game-launch.md](game-launch.md).
- The launcher never writes the game's `settings.json` and never passes the relay address (`backend_url`): that stays the game's own,
  the same for everyone.
- **The start screen.** The game opens its start screen only for no arguments or `--menu`; any other argument, even
  `--name`, starts a world directly. So the plan always begins with `--menu`, unless the extra arguments name where to go
  (a seed first, `--world`, `--join`, `--host` or `--menu` itself). Tested in `launch/independence_tests.rs`, and `tools/check-start-screen.sh <build dir>` checks a real game build headlessly (own Xvfb): no `saves/` appears.
- **The game is independent of the launcher.** It is started in its own session (Unix: `setsid` in a `pre_exec`) or with
  `CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP` and, when the launcher's job object allows it, `CREATE_BREAKAWAY_FROM_JOB`
  (Windows; a refused breakaway is retried without it). `CREATE_NO_WINDOW`, not `DETACHED_PROCESS` (the two cannot be combined):
  a process without any console makes every console program it starts open a console window of its own, and the game starts
  its own exe again (start screen to world) and `cmd /C ver` (`--diagnostics`). The game exe is a GUI program since the build that
  follows 0.1.0-devrun.3, so it also has no console and no `conhost.exe`; with an older console build `CREATE_NO_WINDOW` still keeps the
  console hidden.
- **When the game starts** (Settings → Game, `launcher.json` key `on_game_start`): `minimise` (default), `close` or `keep_open`. The
  game is the same in all three (detached, `running.json`, the log file). Once the game has stayed up for 2 seconds (`game::START_CONFIRM`;
  one that dies at once leaves the launcher in view with its output) the window is minimised, closed, or left alone. A minimised window is
  brought back and focused when the game ends, and the Game output page then shows how it ended; closing and starting the launcher again
  shows a running game from `running.json`. The same on Linux (no terminal is involved on either system). Stop, and a game of an
  earlier launcher run, never minimise.
  No parent-death signal, no kill on drop. Its stdin is closed and stdout and stderr are the log file itself, not pipes: a pipe
  whose reader is gone would end the game with a broken pipe at its next print. The launcher follows `logs/latest.log` with
  a thread for the Game output page.
- **Closing the launcher while the game runs** just closes the launcher; the game keeps running. `game/running.json` (pid,
  program, start time, log file, command) is written at the start and removed when the launcher sees the game end. The next
  launcher start reads it, checks that the pid still is that program (Linux: `/proc`, macOS: `ps`, Windows: `tasklist`) and shows
  "running" again with the last 64 KiB of output, the running time, and Stop. An adopted game has no exit code. A file whose
  game is gone is deleted. Only the Stop button ends a game.
- Output (stdout and stderr) goes to `logs/latest.log` and to the Game output page. A crash is an exit code other than 0
  or a new file in `crash/`; the launcher shows the report.
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

## Launcher self-update

Code: `crates/core/src/selfupdate.rs` and its folder (no UI), `crates/ui/src/launcher.rs` (state, line, restart). The
game's update flow above is separate and unchanged.

- **Check**: once per start, after the game's own check is done (never while the game's update is downloading or being
  swapped), unless offline mode or Settings → General → *Update the launcher automatically* is off. It asks the release
  API for `product=launcher` for the running platform; the channel follows the beta switch (stable, plus beta when it is
  on, the higher version wins). It compares with `CARGO_PKG_VERSION` as semver: only a newer version counts, never a
  downgrade, and a version on the skip list (below) is ignored. Failures (offline, server down) are quiet: the line says it
  could not check.
- **Stage**: the zip goes through the same resumable download as the game (Range, SHA-256), then the signature check
  ([release-api.md](release-api.md#signatures); a launcher is never accepted unsigned), then the zip limits (64 entries,
  512 MB, no `..`, no absolute paths, no symlinks) into `<data>/launcher-update/staged/`. A staged copy of the same
  release is reused.
- **Where the launcher lives decides what happens** (`selfupdate::detect`): if its folder is writable and it is not
  an AppImage, a system folder (`/usr`, `/nix/store`, `/snap`, `/app`, `Program Files`, `WindowsApps`) it replaces itself.
  Otherwise (a *managed install*) nothing is downloaded or replaced: About shows "update available: 0.3.0 (managed
  install)", says why, shows the notes and has a button that opens the download page.
- **Apply on Linux**: the running file may be replaced by a rename. The current file is copied to
  `castanea-launcher.old`, the new one is copied next to it (same filesystem), given the old file's mode and
  renamed over `castanea-launcher` in one atomic step.
- **Apply on Windows**: the running exe cannot be overwritten or deleted but can be renamed. It becomes
  `castanea-launcher.old.exe`, the new exe is copied next to it and renamed to `castanea-launcher.exe`. The running
  process carries on; About and the home page offer **Restart to finish the update** (the next start also runs the new
  one). A step that fails is undone, so the launcher is never left without an exe. A rename that a virus scanner blocks
  is tried 5 times.
- **Both** are written as a list of operations (`apply_plan`, `rollback_plan`) that tests run on any system, with an
  undo of the finished steps when one fails.
- **Rollback** (a file-based state machine, `selfupdate::startup`, first thing in `main`): before applying, the launcher
  writes `<data>/launcher-update/update-pending` (from, to, time). The new launcher stamps `started_at` into it when it
  starts, and the marker is deleted after the third drawn frame, which also deletes `.old`/`.bad`/`.new` (quietly retried
  for 5 seconds if a file is locked). The staged launcher (`staged/`, `staged.json`) is deleted as soon as it is in place,
  so after a good start the folder holds nothing. A marker that is still stamped at the next start means the new launcher never drew
  a frame (it crashed, within seconds of starting): that start restores `.old` over the executable (on Windows the broken
  exe is first renamed to `.bad.exe`), writes the failed version to `skip.json`, deletes the marker, starts the
  restored launcher and exits. A marker for another version than the running one is stale and dropped. With no `.old` to
  restore, the marker and the version are dropped and the launcher runs on. About lists skipped versions.
- **UI**: one quiet line, `castanea launcher 0.2.0 · up to date`, or `update 0.3.0 downloading 43 %`, `restart to update
  to 0.3.0`, `update available: 0.3.0 (managed install)`, `update 0.3.0 refused, not signed by castanea`, offline,
  automatic updates off, could not check (English and German). The home page only gets a quiet *Restart to finish the update*
  link next to What's new. Restart is refused while the game runs.

## Storage and reset

Settings → Storage and reset lists everything the launcher keeps on the machine, each row with its name, the real path, its
size and its own Delete button (`crates/core/src/storage.rs`, the page in `crates/ui/src/pages/storage.rs`).

| Row | What it holds (paths below the three folders above) | Deleted by Delete everything |
|---|---|---|
| Cache | everything in `cache/` (index, images, news, thumbnails, downloads and `.part` files), `data/backend.json` (probe results and the crash memory of Vulkan starts), `data/launcher-update/` (the staged launcher, the update marker, the skip list) | yes |
| Game | `data/build/` (`current`; `next` or `previous` only during an update or left by one that stopped, removed at the next start) and the link `data/game/assets` | yes |
| Mods | `data/game/mods/` (enabled and `.disabled`) | yes |
| Logs | `data/game/logs/` (`latest.log` and the older stamped ones) | yes |
| Launcher settings | everything in `config/` (`launcher.json`, a leftover `github-token`, old `profiles.json`) and `data/window/` (the window state) | yes |
| Game data (worlds, game settings) | what the game wrote in `data/game/` itself: `saves/`, `screenshots/`, `settings.json`, `crash/`, `debug/`. Listed only when the game folder exists | **never** (typed `DELETE`) |

The launcher makes nothing else: no temp folders (an archive unpacks to a sibling `.unpacking` folder that a failure removes),
no registry entries, no Start menu or desktop shortcuts. The Windows zip is two files (`castanea-launcher.exe`, `LICENSE`); updates
leave `castanea-launcher.old.exe`, `.bad.exe` and `.new.exe` next to the exe for a few seconds, which only the uninstall removes.
The game itself keeps nothing outside `data/game/`.

Rules, enforced in the core and tested with temp folders:

- A path is deleted only when its parent resolves (symlinks followed there) to a strict child of the config, data or cache folder;
  `..`, the folders themselves and anything else are refused. A folder you added by path (a local build folder, `--game-dir`) is never
  a target; Delete everything removes only the reference (it is a setting) and refuses a target that holds or lies inside it.
- Links are removed, never followed: a symlink or junction in `mods/` or `build/` that points out of the launcher's folders loses
  only the link. The same walk measures sizes, and a link counts as zero.
- Nothing is deleted while the game runs, while a download or update is in progress, or while news are loading; the buttons say why.
- A file that cannot be removed (on Windows: open in another program, a virus scanner) is reported with its path and the system's
  message under its row, with *Try again*; the other items and rows still go. A read-only file loses the flag and is tried once more.
- Sizes are measured on one thread per row after the page is first shown (`…` until the answer arrives), so the page never waits for
  the disk. A row with nothing on disk shows `–` and has no active Delete.
- Every Delete asks first, listing the paths. Game data also asks for the word `DELETE` typed exactly (spaces around it are ignored);
  it is not part of Delete everything, not even as an option.
- After Delete everything the launcher says so and closes (its settings are gone, and anything it kept saving would bring them back);
  the next start is a first start. Deleting only Launcher settings resets the running launcher to the defaults; deleting only Game
  leaves it without a game and without an automatic download until you press Play or check again.

### Uninstall (Windows only)

*Uninstall launcher* (`cfg(windows)`, absent on Linux and macOS) asks once, listing what goes, then: deletes the data rows as
Delete everything does (a failure stops it, with the per-row errors and *Try again*), starts a copy of the launcher from the temp folder
as `castanea-uninstall-<pid>.exe --finish-uninstall <exe> <pid>`, and exits without saving. The helper waits for that pid to end
(`tasklist`), sweeps the data again (the window state may have been written on exit), removes the shortcuts, `.old`/`.bad`/`.new`
files, the shipped `LICENSE`, and the exe (retried for 20 s: a virus scanner may hold it), then the install folder if it is empty, and
finally deletes itself through a short `cmd /c` (`MoveFileExW(… DELAY_UNTIL_REBOOT)` if that cannot start; that call needs admin
rights and is only a fallback, a copy left in `%TEMP%` is harmless).
The plan is a pure function (`storage::uninstall_plan`) so it is tested everywhere:

- The exe's folder is **owned** when its name starts with `castanea-launcher` and it is not one of the shared folders (user folder,
  Downloads, Desktop, Documents, Program Files, `%TEMP%`, `%APPDATA%`, `%LOCALAPPDATA%`): then the exe, its leftovers and `LICENSE` go, and
  the folder when it is empty after that (a file of yours in it keeps it). Otherwise only the exe and its leftovers are removed, the folder
  stays, and the question says so.
- Shortcuts: `castanea-launcher.lnk`, `Castanea Launcher.lnk` or `Castanea.lnk` in the per-user Start menu `Programs` folder or on
  the desktop, when they exist (the launcher makes none; these are the names a person gives a hand-made one). Registry: none, the launcher
  writes no keys (it only reads `HKCU\Control Panel\International`).
- Worlds and game settings stay, as in Delete everything.

## Security model

- Downloads use HTTPS only. Plain `http://` is refused, except to `127.0.0.1` and `localhost` when a test asks for it.
- Every download has a SHA-256 from a trusted place (the index, or the release API) and a size limit; a
  mismatch deletes the file and keeps the installed build.
- **Releases are signed.** After the hash, the game and the launcher downloads are verified with Ed25519 against keys
  embedded in the launcher ([release-api.md](release-api.md#signatures)). An unsigned, wrongly signed or unknown-key
  release is refused and nothing is unpacked. The key list is compiled in; `CASTANEA_TRUST_KEYS_FILE` works in debug builds only.
  The private key stays on the build host. The mod index's own signature (`docs/mod-index.md`) is still planned.
- The only downloaded code the launcher runs is the game binary, and its own new version after a signed, hash-checked
  self-update. Nothing from a mod.
- The launcher never runs anything from a mod. Mods are
  untrusted data: archives are unpacked by the game, not by the launcher, but are checked for unsafe paths on import.
  Wasm in a mod runs only inside the game's sandbox; native code cannot come with a mod (Rust `ModApi` mods are
  compiled into the game).
- A new mod lands disabled. Enabling shows what it declares: its side, whether it carries Wasm code, its dependencies,
  aliases and file count.
- No secret is stored: the release API needs no token.

## Open decisions for the user

- Whether the game should print `--version` or ship `version.txt`.
- Whether `mod.json` should get a `game` key (a version range) so the range does not depend on the index.
- Where the public index lives (a file in a repo, the backend, a CDN) and who signs it.
- Authenticode signing of the Windows exe (SmartScreen) and an MSI: see [packaging.md](packaging.md).
