# Windows test, 2026-11-18

A real Windows 11 PC (Pro 10.0.26300, 1920x1080 at 150 % scaling, Intel UHD Graphics with driver 31.0.101.2114 and a virtual display driver). The checklist is
[windows.md](windows.md) items 1 to 22 and [game-launch.md](game-launch.md). Nothing was published; the live release API was never written to.
The launcher was started once without a local server by mistake (the first crash investigation), which made one read-only `GET` of `latest` and `news`
against the live API, then every run used a local server.

## How it was built and run

- Launcher `bdf9a82` built on the build host: `tools/release.sh windows-x86_64 0.1.0 --allow-dirty` (mingw cross build, release exe, 20 MB, `--version` says `castanea-launcher 0.1.0`).
  A release build ignores `CASTANEA_TRUST_KEYS_FILE`, so the server flows need a **debug-assertions** build. Those were built the same way with
  `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build --release -p castanea-launcher --target x86_64-pc-windows-gnu`, as 0.1.0 (old), 0.2.0 (new, `Cargo.toml` and
  `Cargo.lock` bumped in a scratch tree only) and 0.3.0.
- A local server, [tools/test_release_server.py](../tools/test_release_server.py) (new in this branch, loopback only, signs with the public test key `seed 00 01 .. 1f`):
  `python3 tools/test_release_server.py MANIFEST.json --port 8765`, `--pubkey` prints the key for `CASTANEA_TRUST_KEYS_FILE`. A manifest is a list of
  `{"product","version","platform","channel","zip","sig":"valid|missing|wrongkey","notes"}`. It serves `latest`, the list and the download with `Range`.
- Environment per run: `CASTANEA_RELEASE_URL=http://127.0.0.1:PORT`, `CASTANEA_TRUST_KEYS_FILE=<file with the test key>`, `CASTANEA_LAUNCHER_HOME=<scratch>`.
- The game was the real Windows build of castanea `0.1.0` (commit `0eae9676`, `--features wgpu-backend`, mingw cross build), zipped with its assets and served as the game release.
- The UI was driven through Windows UI Automation (the launcher exposes AccessKit) and real mouse clicks from a PowerShell script; a DPI-aware process is needed, or the
  coordinates are off by the 150 % scaling.

## Results

| # | Item | Result |
|---|---|---|
| 1 | Starts, window frame | **Crashes at start on this PC with the code as it was** (see bug 1); works with the fix. Frame: caption, thick frame, system menu, minimise and maximise boxes (`WS_OVERLAPPEDWINDOW`), title `castanea`, no console. Size and position are kept across a restart (1500x900 at 0,0 came back). Snapping, IME and Narrator were not tried. |
| 2 | First start online, Play, output, junction | Works. The status line went from nothing to `castanea 0.1.0 · up to date`, the game zip was downloaded from the local server and its signature accepted, `data\game\assets` is a junction to `data\build\current\assets` (`LinkType: Junction`). Play shows `checking graphics…`, then `Stop the game, running for 0:11`; the game ran 5 minutes without a crash; Stop (UI Automation `Invoke`) ended it, no process was left. |
| 3 | Update while the game runs | Partly: the launcher update was staged while the game ran. The *game* update swap was not tried (one game version on the server). |
| 4 to 14 | Offline, crash message, mods import, local build folder, release server setting, text input and IME, file dialogs, Narrator, odd paths, HiDPI, old profiles | Not tried. HiDPI at 150 % drew correctly (see bug 3 for the window height); the release server setting is only shown (the field exists with Save). |
| Game folder | Game folder detection | The settings page shows `...\data\game`; the launcher started the game from `data\build\current\castanea.exe` in that folder. |
| Probe flow | `castanea.exe --probe-gpu` through the launcher | Works with the real game: the probe ran, `backend.json` holds the report (`vulkan` ok, `wgpu` ok, adapter Intel UHD, driver `Intel Corporation Intel driver`), the decision was `vulkan` (`vulkan_works`), the game log says `graphics backend: vulkan`. About › Diagnostics shows game 0.1.0, Graphics setting, `Vulkan`, `Vulkan passed the check before the start`, `Intel(R) UHD Graphics · Intel Corporation Intel driver`. |
| Settings | Persistence, Graphics setting | Works. Settings › Advanced › Graphics has Auto, Vulkan, wgpu; choosing `wgpu` wrote `"graphics": "wgpu"` into `config\launcher.json`, cleared the probe cache (as documented), survived the launcher restart, and the next Play started the game with `CASTANEA_BACKEND=wgpu` (`reason: setting`). With the game as it was that start crashed (game bug, fixed in the game branch); with the fixed game it opened the start screen on DX12 (`wgpu: surface Bgra8UnormSrgb, present Immediate`) and the window drew correctly. |
| 15 | Rename trick | **Works.** After the check the launcher downloaded 0.2.0 while running: `castanea-launcher.exe` (0.2.0 by `--version`) and `castanea-launcher.old.exe` (0.1.0, still running) next to each other, `update-pending` written, no error. |
| 16 | Restart prompt | **Works.** With the game running the link says `The game is running. Close it, then restart the launcher.` and does not restart. With the game closed it closed the window and opened the new launcher; About says `Version 0.2.0 · up to date`. |
| 17 | Cleanup | **Works.** About 13 s after the restart `castanea-launcher.old.exe` was gone and `update-pending` was removed. Defender raised no detection (`Get-MpThreatDetection` empty, item 18). |
| 18 | Defender, SmartScreen | No Defender detection and no blocked rename; no SmartScreen prompt was seen (nobody watched the screen, so this is only the absence of a blocked step). The first start of a freshly copied game exe took 8.5 s (scan), later starts under 1 s. |
| 19 | Rollback | **Works.** A 0.3.0 built from the unfixed source (it really crashes before its window): first start exited with `-1073741819`, second start printed `castanea-launcher: 0.3.0 did not start, back to 0.2.0`, `castanea-launcher.exe` is 0.2.0 again, `skip.json` lists `0.3.0`, About says `Skipped because it did not start: 0.3.0` and `up to date`. A stand-in exe that is not a launcher (`whoami.exe`) cannot be rolled back, which is expected: the rollback runs in the launcher's own start code; the page's wording `a program that exits` should say `a launcher build that fails at start`. |
| 20 | Refused downloads | **Works** for an unsigned 0.4.0 and for one signed by another key: About says `update 0.4.0 refused, not signed by castanea` and `The new launcher was not signed by castanea and was thrown away. The installed launcher stays.`; no file was added; the game was still installed from the same server. |
| 21 | Read-only install | **Works, approximated.** `Program Files` needs admin, so the exe's folder was made unwritable with an ACL deny: About says `update available: 0.2.0 (managed install)`, `This launcher's folder is read-only for you, so it cannot replace itself. Download the new version.` and the button `Open the download page`. The button itself was not pressed. |
| 22 | Locked old file | **Works.** A stale `castanea-launcher.old.exe` held open exclusively: the launcher said `could not check for updates`, added no file and left `castanea-launcher.exe` at 0.1.0. After the lock was released the next start downloaded 0.2.0 and swapped (exe 0.2.0, old file renamed to `.old.exe`). The launcher was never left without its exe. |

## Bugs found

1. **The launcher crashes at start on an Intel UHD PC** (`castanea-launcher.exe`, access violation in `igvk64.dll`, the Intel Vulkan driver; every start, release and debug build alike, no window).
   eframe's default asks wgpu for all primary backends and on this machine wgpu lists the Vulkan adapter first. `docs/windows.md` says `wgpu picks DX12, then Vulkan, then GL`, which was not what the code did.
   Repro: start the 0.1.0 release exe on a machine with that driver. `WGPU_BACKEND=dx12` makes it work. **Fixed** in `crates/ui/src/main.rs`: on Windows the instance backends are
   `Backends::from_env().unwrap_or(DX12 | GL)`; `WGPU_BACKEND` still wins. Checked: `tools/winck.sh` passes (check and clippy `-D warnings`), `cargo fmt --check` clean, the fixed exe starts and everything above ran on it.
2. **The release build ignores the trust key, so the update flow cannot be tried with a release exe** (by design). There was no standalone test server in the repo (`release_server.rs` is test-only). Added `tools/test_release_server.py`.
3. **The start window is taller than a 1080p screen at 150 % scaling**: the start size is 1115x737 logical, 1106 physical pixels, the screen is 1080, so the bottom of the window and the `About` entry at the bottom of the sidebar are off the screen until the window is moved or resized. Windows leaves a window that size as it is. Open: clamp the start size to the work area of the monitor (the size is persisted after the first run).
4. **`castanea-launcher --screenshots DIR` exits 1 on Windows**: four pages are written, then `screenshots: home-running: never settled (size [1280.0 720.0])`. Open, not looked into.
5. `windows.md` item 19 said `a program that exits at once`; a real launcher build that fails is needed (see item 19). Fixed in the doc.
6. About › Diagnostics shows the graphics card with the Vulkan driver only: the game's wgpu probe object has no `driver` field (`backend.json` says `"driver": ""` for wgpu). Cosmetic, a game-side field.

## Not verified

Items 4 to 14 of the list, the game update swap while the game runs (3), snapping, IME, file dialogs, Narrator, HiDPI at 125 and 200 %, a second display.

## Commands

```powershell
# in WSL, from a worktree of this repo
tools/release.sh windows-x86_64 0.1.0 --out DIST --allow-dirty          # release exe for the UI checks
python3 tools/test_release_server.py MANIFEST.json --port 8765           # in another terminal
$env:CASTANEA_RELEASE_URL='http://127.0.0.1:8765'
$env:CASTANEA_TRUST_KEYS_FILE='keys.txt'                                 # the output of --pubkey
$env:CASTANEA_LAUNCHER_HOME='C:\scratch\cl-home'
.\castanea-launcher.exe                                                  # a debug-assertions build for the signed flows
```
