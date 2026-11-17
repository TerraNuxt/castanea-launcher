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
| Updating the launcher | the running exe is renamed to `castanea-launcher.old.exe` and the new exe takes its name (a running exe can be renamed, not overwritten or deleted); a restart finishes it and the old file is deleted after the new launcher drew a few frames ([design.md](design.md#launcher-self-update)) |
| Starting the game | `CREATE_NO_WINDOW`, so no console window flashes; output is still captured |
| Game binary | `castanea.exe` |
| Release | platform `windows-x86_64` of the release API; the zip holds `castanea.exe` and `assets` |
| System language | `LANG` if set, else `HKCU\Control Panel\International\LocaleName` |
| Renderer | wgpu picks DX12, then Vulkan, then GL |
| Window frame | the system frame (snapping, shadow and resizing are Windows' own) |
| Open folder | `explorer <folder>` |

## Not verified on a real Windows machine yet

Please try these on a Windows PC and report what fails:

1. `cargo run --release -p castanea-launcher` opens the window with no console behind it; resize, move, snap it to a
   screen edge and close it, start again: size and position are kept (`%LOCALAPPDATA%\castanea-launcher\window`).
2. A first start online: the status line says it downloads the newest release with a percentage and the speed, then `up to date`;
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
8. Settings → Advanced → Release server and Include beta builds: the default is `https://castanea.xenoo.dev`; with beta on, a newer beta build is downloaded. An `http://` address (other than the loopback) is refused. Cut the network in the middle of a download and restore it: the download continues from where it stopped (the status percentage does not start at 0), also after closing and starting the launcher. "What's new" shows the release notes.
9. Text input: type in the search field and the settings fields, use an IME (for example Japanese), paste with Ctrl+V.
10. The file dialogs (import, local build folder, local index file) open the native Windows dialogs.
11. Screen readers: Narrator reads the sidebar items (as tabs), the Play/Stop button with the running time, the icon
    buttons (refresh, trash) and the switches by name (AccessKit).
12. A path with spaces and non-ASCII letters for `CASTANEA_LAUNCHER_HOME` and for the local build folder.
13. HiDPI: at 125 %, 150 % and 200 % display scaling the text and icons are sharp, the hairlines stay 1 px, nothing
    overlaps; the window cannot be made smaller than 1000x640.
14. An old launcher home with profiles (from before this version): the first start keeps the last used profile's
    saves and options, says where the other profile folders stayed, and the old `data\versions` builds are gone.

### Launcher self-update (needs a launcher release on a test server)

Set up: copy `castanea-launcher.exe` of an older version (say 0.1.0) to a writable folder such as
`%USERPROFILE%\Downloads\cl-test`, publish a newer launcher (0.2.0, `windows-x86_64`) to a test release server, and point the
old one at it with `set CASTANEA_RELEASE_URL=http://127.0.0.1:PORT` (loopback `http` works) and a test key list
(`CASTANEA_TRUST_KEYS_FILE`, debug builds only) or a server that signs with the real key. Use
`CASTANEA_LAUNCHER_HOME=C:\cl-home` so nothing touches your real data.

15. **The rename trick.** Start the old exe. After the check the line in About changes to `update 0.2.0 downloading …`, then
    `restart to update to 0.2.0`. In the folder next to the running exe there are now `castanea-launcher.exe` (the new one,
    version 0.2.0 with `--version`) and `castanea-launcher.old.exe` (the old, still running). No "access denied", no error toast.
16. **The restart prompt.** The button *Restart to finish the update* (About and next to What's new on the home page) closes
    the window and opens the new launcher; About says `up to date` with 0.2.0. While the game runs the button says to close
    the game first and does not restart.
17. **Cleanup.** After the new launcher has drawn a few frames, `castanea-launcher.old.exe` disappears within a few seconds
    (it may take a moment if Defender is scanning it). `%LOCALAPPDATA%\castanea-launcher\data\launcher-update\` has no
    `update-pending` file any more. Closing the launcher before the new one ran and starting the exe by hand
    also runs 0.2.0 (the update does not need the button).
18. **Antivirus and SmartScreen.** Note whether Defender or SmartScreen warns about the new exe or blocks the rename;
    the code retries a blocked rename 5 times. An exe without Authenticode signing may get a SmartScreen prompt on the
    first start of a downloaded copy; that is expected until it is signed.
19. **Rollback.** Publish a 0.3.0 whose exe exits at once (or replace the staged exe in
    `data\launcher-update\staged` by a program that exits, before the restart). Restart: the window never opens, start
    again: the old 0.2.0 is back (`castanea-launcher.exe` is the old one, the broken one is `castanea-launcher.bad.exe` and
    goes away later), About lists `0.3.0` as skipped and no update is offered for it.
20. **Refused downloads.** Publish a launcher 0.4.0 without a signature, or signed by another key: About says `update 0.4.0
    refused, not signed by castanea`, a toast says the same, the folder has no new files, and the game's update still works.
21. **Read-only install.** Put the exe in `C:\Program Files\castanea-launcher` (needs admin to copy): it does not try to
    replace itself, About says `update available: 0.2.0 (managed install)`, explains why, and *Open the download page*
    opens the browser.
22. **Locked old file during apply.** Hold a stale `castanea-launcher.old.exe` open (for example `copy` the old exe to that
    name and open it in a program that locks it) and start an update. Expected: the update fails cleanly (a quiet
    `could not check for updates` line, no new files, the installed exe untouched) and succeeds on the next start once the lock
    is gone. Report if the launcher is ever left without `castanea-launcher.exe`.
