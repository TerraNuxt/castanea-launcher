# Windows

The launcher builds for `x86_64-pc-windows-msvc` (the target to ship) and is cross-checked for `-gnu` and `-msvc`
from Linux with `tools/winck.sh` (cargo check and clippy with `-D warnings` in a container; nothing is linked or run).
CI runs clippy, the tests and a release build on `windows-latest`.

## What differs from Linux

| Area | Windows |
|---|---|
| Folders | config `%APPDATA%\castanea-launcher`, data and cache `%LOCALAPPDATA%\castanea-launcher` |
| The build's `assets/` in the game folder | a directory junction (no admin rights, no developer mode); Linux uses a symlink |
| Updating the build | the swap of `build\next` and `build\current` waits until the game has ended (Windows cannot move a folder with a running program in it); a rename or delete that a virus scanner blocks is tried again 5 times, quietly; `build\previous` still locked afterwards is removed at the next start. Only `build\current` is left after an update ([design.md](design.md#the-game-build)) |
| Updating the launcher | the running exe is renamed to `castanea-launcher.old.exe` and the new exe takes its name (a running exe can be renamed, not overwritten or deleted); a restart finishes it and the old file is deleted after the new launcher drew a few frames ([design.md](design.md#launcher-self-update)) |
| Starting the game | `DETACHED_PROCESS` + `CREATE_NEW_PROCESS_GROUP` (+ `CREATE_BREAKAWAY_FROM_JOB` when the job allows it): no console, no Ctrl+C shared with the launcher, not in the launcher's job; output goes to `logs\latest.log` directly; closing the launcher leaves the game running |
| Game binary | `castanea.exe` |
| Release | platform `windows-x86_64` of the release API; the zip holds `castanea.exe` and `assets` |
| System language | `LANG` if set, else `HKCU\Control Panel\International\LocaleName` |
| Renderer of the launcher | wgpu with DX12 and GL only (`WGPU_BACKEND` overrides): wgpu's own pick reached a crashing Intel Vulkan driver |
| Renderer of the game | the launcher probes the game (`--probe-gpu`) and passes `CASTANEA_BACKEND=vulkan` when Vulkan works, else `wgpu` (DX12) ([game-launch.md](game-launch.md)) |
| Window frame | the system frame (snapping, shadow and resizing are Windows' own) |
| Open folder | `explorer <folder>` |
| Storage and reset | Settings lists the launcher's data with sizes and Delete buttons; *Uninstall launcher* (Windows only) deletes the data, then the exe through a helper copy in `%TEMP%` that waits for the launcher to exit ([design.md](design.md#storage-and-reset)) |

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
   Afterwards `%LOCALAPPDATA%\castanea-launcher\data\build` holds only `current` (no `previous`, no `next`).
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
15. The start screen: with a player name set, Play opens the game's start screen (world list), not a world. The Game output
    first line shows `castanea.exe --menu --name <name>`.
16. Close the launcher while the game runs (window close button, and also End task in Task Manager on the launcher): the game
    window stays open and playable, with no console window, also after it prints (Game output keeps going in
    `data\game\logs\latest.log`). Start the launcher again: it shows the game as running with its time and Stop, and the
    Game output page shows the last lines. Stop ends it. Check in Task Manager's details view that the game is not a child
    of the launcher (Process Explorer: tree view) and that Ctrl+C in a console that started the launcher does not end the game.
17. Start the launcher from a job that kills its children on close (for example a PowerShell started by a scheduled task,
    or a CI runner): the game must still survive the close of the launcher; if the job forbids breakaway the game then starts
    without that flag (log line in `latest.log` is not needed) and the result of this test is worth reporting.

### Launcher self-update (needs a launcher release on a test server)

Set up: copy `castanea-launcher.exe` of an older version (say 0.1.0) to a writable folder such as
`%USERPROFILE%\Downloads\cl-test`, publish a newer launcher (0.2.0, `windows-x86_64`) to a test release server, and point the
old one at it with `set CASTANEA_RELEASE_URL=http://127.0.0.1:PORT` (loopback `http` works) and a test key list
(`CASTANEA_TRUST_KEYS_FILE`, debug builds only) or a server that signs with the real key. Use
`CASTANEA_LAUNCHER_HOME=C:\cl-home` so nothing touches your real data. A local server is `tools/test_release_server.py`
(`python3 tools/test_release_server.py MANIFEST.json --port 8765`, `--pubkey` prints the test key for the key file); the signed flows
need a debug-assertions build. Items 15 to 17 and 19 to 22 were run this way on 2026-11-18: [windows-test-2026-11-18.md](windows-test-2026-11-18.md).

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
19. **Rollback.** Publish a 0.3.0 that is a real launcher build failing at start (the rollback runs in the launcher's own start code, so
    a foreign program such as `whoami.exe` cannot be rolled back). Restart: the window never opens, start
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

### Storage, reset and uninstall (new, not run on Windows yet)

Set up: use `CASTANEA_LAUNCHER_HOME=C:\cl-home` for items 23 to 28 so nothing touches your real data, and a copy of
`castanea-launcher.exe` in `C:\cl-test\castanea-launcher\` (an own folder) and another in `%USERPROFILE%\Downloads` (a shared one).
Design and rules: [design.md](design.md#storage-and-reset).

23. **The list.** Settings → Storage and reset shows Cache, Game, Mods, Logs, Launcher settings and (after one run of the game) Game
    data, each with its real path (`%LOCALAPPDATA%\castanea-launcher\…`, `%APPDATA%\castanea-launcher`), a size that appears a moment
    after the page (the page never stalls, also with a big world) and a Delete button. **Uninstall launcher** is there; on Linux it is not.
24. **One row.** Delete Logs and Mods: the question lists the paths, then they are gone in Explorer and the sizes read `–`. Delete Game:
    the status line on Play says the game is not installed and nothing downloads by itself; Play or a restart installs it again.
25. **Locked files.** Start the game, then Delete: the buttons are off and say to close the game. With the game closed, open a log
    in an editor that locks it (or open a file from `build\current` in a program that locks it) and press
    Delete everything: the other rows still go, the locked item stays under its row with the path and Windows' message, and *Try again*
    finishes it once the lock is closed. Also try a read-only file in the cache (`attrib +r`): it is deleted.
26. **Junctions.** `data\game\assets` is a junction to `build\current\assets`: Delete Game removes the junction and the build and
    leaves nothing dangling. Make a junction inside `mods\` to a folder with a test file (`mklink /J`) and delete Mods: the junction
    goes, the test file in the target folder is still there. Set a local build folder (Advanced) and Delete everything: that
    folder is untouched; only the setting is gone.
27. **Game data.** Delete on the Game data row asks for `DELETE` (typed in capitals; `delete` and `DELET` keep the button off) and then
    removes `saves` and the game's settings. Delete everything never touches them: the worlds are still there afterwards.
28. **Delete everything.** One question listing every path; after it the launcher says it closes, closes, and the next start is a first
    start (language from the system, no game, no mods). The window state is not written again after the deletion.
29. **Uninstall, own folder.** Run `C:\cl-test\castanea-launcher\castanea-launcher.exe` (own `CASTANEA_LAUNCHER_HOME`), Uninstall: the
    question lists the rows, the program and says the folder is removed when empty. After confirming the window closes within a
    second; within about 5 seconds the data, the exe, `LICENSE` and the folder `castanea-launcher` are gone, no `castanea-uninstall-*.exe`
    is left in `%TEMP%` (if one is, note whether Defender held it). A file of yours put into that folder before keeps the folder.
30. **Uninstall, shared folder.** The same from `Downloads\castanea-launcher.exe`: the question says only the program is removed and the
    folder stays; afterwards `Downloads` and its other files are untouched and the exe is gone.
31. **Uninstall while locked.** Hold a file in the data folder open and Uninstall: the exe is not removed, the error shows under the row,
    and after closing the lock *Try again* and Uninstall work. Hold the exe itself open from a second process (copy it and run the
    copy with `--version` in a loop): the helper keeps trying for 20 s, then reports nothing visible; check what is left.
32. **Windows features.** Antivirus: note whether Defender or SmartScreen reacts to the temp copy `castanea-uninstall-<pid>.exe`.
    Screen reader: the Delete buttons are announced as `Delete Cache` and so on, the row's size is read, the DELETE field is labelled.

### Removing the launcher by hand

If Uninstall fails, close the launcher and the game, then delete these (nothing else belongs to the launcher):

- `%LOCALAPPDATA%\castanea-launcher` (the data, the installed game builds, the cache, the window state) and `%APPDATA%\castanea-launcher`
  (`launcher.json`). Your worlds are in `%LOCALAPPDATA%\castanea-launcher\game\saves` (portable mode:
  `<home>\data\game\saves`); copy them away first if you want to keep them.
- The folder you unpacked the zip into, or just `castanea-launcher.exe`, `LICENSE` and any `castanea-launcher.old.exe`,
  `.bad.exe`, `.new.exe` next to it.
- A shortcut you made yourself. The launcher writes no registry keys, so there is nothing to clean there.
- If `%TEMP%\castanea-uninstall-*.exe` is left over, delete it too.
