# Windows

The launcher builds for `x86_64-pc-windows-msvc` (the target to ship) and is cross-checked for `-gnu` and `-msvc`
from Linux with `tools/winck.sh` (cargo check and clippy with `-D warnings` in a container; nothing is linked or run).
CI runs clippy, the tests and a release build on `windows-latest`.

## What differs from Linux

| Area | Windows |
|---|---|
| Folders | config `%APPDATA%\castanea-launcher`, data and cache `%LOCALAPPDATA%\castanea-launcher` |
| The build's `assets/` in a profile folder | a directory junction (no admin rights, no developer mode); Linux uses a symlink |
| Starting the game | `CREATE_NO_WINDOW`, so no console window flashes; output is still captured |
| Game binary | `castanea.exe` |
| Release asset | `castanea-<version>-x86_64-pc-windows-msvc.zip` (or `...-windows.zip`) |
| System language | `LANG` if set, else `HKCU\Control Panel\International\LocaleName` |
| Renderer | wgpu picks DX12, then Vulkan, then GL |
| Open folder | `explorer <folder>` |

## Not verified on a real Windows machine yet

Please try these on a Windows PC and report what fails:

1. `cargo run --release -p castanea-launcher` opens the window with no console behind it; resize, move and close it,
   start again: size and position are kept (`%LOCALAPPDATA%\castanea-launcher\window`).
2. Text input: type in the search field, use an IME (for example Japanese), paste with Ctrl+V.
3. Game versions → Add folder…: pick a castanea checkout with `target\release\castanea.exe` built, or a folder with
   `castanea.exe` next to `assets\`. Play: the game starts, the log shows its output, closing the game shows
   "ended normally". Check that `%LOCALAPPDATA%\castanea-launcher\data\instances\default\assets` is a junction
   (`dir /AL`) and that deleting the profile folder does not touch the build.
4. A crash: start the game with an argument it refuses (Profiles → Extra arguments `--world`) and check that
   the launcher says it crashed and shows the output.
5. Mods → Import zip…: a zip made with `castanea-mod pack`; it lands in `mods\.disabled`, enabling moves it to `mods\`.
   Open mods folder opens Explorer.
6. Settings → GitHub token, then Game versions → Releases with a private game repo (once releases exist).
7. The file dialogs (import, add folder, local index file) open the native Windows dialogs.
8. Screen readers: Narrator reads the sidebar items (as tabs), the icon buttons (gear, pencil, trash, window buttons) and the switches by name (AccessKit).
9. A path with spaces and non-ASCII letters for `CASTANEA_LAUNCHER_HOME` and for the game folder.
10. The launcher's own window frame (the default): drag the title bar to move the window, double click it to maximise,
    use minimise, maximise and close at the top right, and resize from every edge and corner. Aero Snap (dragging to
    a screen edge) and the window shadow may be missing with an undecorated window; Settings → Window frame →
    "Use the system's window frame" switches to the normal Windows frame at once.
11. HiDPI: at 125 %, 150 % and 200 % display scaling the text, icons and the banner are sharp and nothing overlaps;
    the window cannot be made smaller than 1000x640.
12. Two profiles running at once: Play on one card, then Play on another; both cards get the green outline, Game
    output has one chip per profile, and Stop ends only that game.
