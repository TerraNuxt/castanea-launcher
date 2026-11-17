# Windows

The launcher builds for `x86_64-pc-windows-msvc` (the target to ship) and is cross-checked for `-gnu` and `-msvc`
from Linux with `tools/winck.sh` (cargo check and clippy with `-D warnings` in a container; nothing is linked or run).
CI runs clippy, the tests and a release build on `windows-latest`.

## What differs from Linux

| Area | Windows |
|---|---|
| Folders | config `%APPDATA%\castanea-launcher`, data and cache `%LOCALAPPDATA%\castanea-launcher` |
| The build's `assets/` in the game folder | a directory junction (no admin rights, no developer mode); Linux uses a symlink |
| Updating the build | the swap of `build\next` and `build\current` waits until the game has ended (Windows cannot move a folder with a running program in it); a rename that a virus scanner blocks is tried again 5 times |
| Starting the game | `CREATE_NO_WINDOW`, so no console window flashes; output is still captured |
| Game binary | `castanea.exe` |
| Release asset | `castanea-<version>-x86_64-pc-windows-msvc.zip` (or `...-windows.zip`) |
| System language | `LANG` if set, else `HKCU\Control Panel\International\LocaleName` |
| Renderer | wgpu picks DX12, then Vulkan, then GL |
| Window frame | the system frame (snapping, shadow and resizing are Windows' own) |
| Open folder | `explorer <folder>` |

## Not verified on a real Windows machine yet

Please try these on a Windows PC and report what fails:

1. `cargo run --release -p castanea-launcher` opens the window with no console behind it; resize, move, snap it to a
   screen edge and close it, start again: size and position are kept (`%LOCALAPPDATA%\castanea-launcher\window`).
2. A first start online: the status line says it downloads the newest release with a percentage, then `up to date`;
   Play starts the game, the Game output page shows its output, closing the game shows "Ended normally".
   `%LOCALAPPDATA%\castanea-launcher\data\game\assets` is a junction (`dir /AL`) to `data\build\current\assets`.
3. An update while the game runs: start the game, publish (or fake with a local folder of releases) a newer release,
   restart the launcher while the game is still running: the download finishes, the line says the new version starts
   next time, and after closing the game the swap happens with no error (no "access denied" from a locked folder).
4. Offline: Settings → Offline mode on, restart: no network request, `offline` in the status line, Play still works.
   Pull the network cable instead: the status line says it could not check, Try again works once the network is back.
5. A crash: Settings → Extra arguments `--world` (a value the game refuses) and Play: the launcher says it crashed and
   shows the output.
6. Mods → Import zip…: a zip made with `castanea-mod pack`; it lands in `data\game\mods\.disabled`, enabling moves it
   to `mods\`. Open mods folder opens Explorer.
7. Settings → Advanced → Local build folder: pick a castanea checkout with `target\release\castanea.exe` built; the
   status line says `Local build …`, Play starts it in place; Use releases switches back. The same with
   `castanea-launcher.exe --game-dir C:\path\to\checkout`.
8. Settings → Advanced → GitHub token while the game repo is private: the update check finds the releases.
9. Text input: type in the search field and the settings fields, use an IME (for example Japanese), paste with Ctrl+V.
10. The file dialogs (import, local build folder, local index file) open the native Windows dialogs.
11. Screen readers: Narrator reads the sidebar items (as tabs), the Play/Stop button with the running time, the icon
    buttons (refresh, trash) and the switches by name (AccessKit).
12. A path with spaces and non-ASCII letters for `CASTANEA_LAUNCHER_HOME` and for the local build folder.
13. HiDPI: at 125 %, 150 % and 200 % display scaling the text and icons are sharp, the hairlines stay 1 px, nothing
    overlaps; the window cannot be made smaller than 1000x640.
14. An old launcher home with profiles (from before this version): the first start keeps the last used profile's
    saves and options, says where the other profile folders stayed, and the old `data\versions` builds are gone.
