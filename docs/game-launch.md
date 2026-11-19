# Starting the game: the graphics backend

The game has two renderers: raw Vulkan (the default, fastest) and wgpu (DX12 on Windows; the fallback). The game picks with
its setting `backend = auto | vulkan | wgpu`, and the environment variable `CASTANEA_BACKEND=vulkan|wgpu` beats the setting.
The launcher uses **Vulkan first, but only when it really works here, else wgpu**, and tells the game with
`CASTANEA_BACKEND`. This page is the contract between the launcher and the game, and the launcher's rules. Code:
`crates/core/src/backend.rs` and its folder.

## What the launcher does at Play

1. **Settings → Advanced → Graphics** decides: `Auto (recommended)`, `Vulkan` or `wgpu`. A chosen backend is passed as
   `CASTANEA_BACKEND` and nothing is probed. `CASTANEA_BACKEND` in the extra environment beats the setting.
2. **Auto**, in this order (the first rule that applies wins):
   1. Vulkan died within seconds of a start on this game version and GPU before: use `wgpu` (see *Crash memory*).
   2. A fresh cached probe for this game version and GPU: use it.
   3. Otherwise run the probe (below), cache the answer, then decide.
3. **The decision** from a probe report: `vulkan.ok` → `CASTANEA_BACKEND=vulkan`; else `wgpu.ok` (or `wgpu` not
   reported) → `CASTANEA_BACKEND=wgpu`; else no start, and a message with both errors (Settings → Advanced → Graphics lets the
   user force one). A probe that is not supported (below) → the game starts with **no** `CASTANEA_BACKEND` (its own `auto`).
4. The probe runs on a worker thread, so the window stays alive; Play shows "checking graphics…" while it runs.

## The probe: `castanea --probe-gpu` (the game side, to be built)

The launcher runs `<game binary> --probe-gpu` in the game folder, with the user's extra environment, stdin closed, no console
window on Windows, and the working directory the game normally has. The game must:

- **Open no window and show nothing.** It creates a Vulkan instance and device, may create a small offscreen image and clear it, and
  may present nothing. It does the same for wgpu (an adapter and a device on the backend wgpu would pick). No audio, no
  save or settings file is read for writing, no network.
- **Print exactly one JSON line to stdout and exit**, within a few seconds (the launcher waits 8 s, then kills it):

  ```json
  {"vulkan":{"ok":true,"adapter":"NVIDIA GeForce RTX 4070","driver":"NVIDIA 550.54.14 (Vulkan 1.3.277)"},
   "wgpu":{"ok":true,"adapter":"NVIDIA GeForce RTX 4070","driver":"Dx12"}}
  ```

  (one line in reality). Per backend: `ok` (bool, required), `adapter` (the device name), `driver` (a driver or API string), and
  `error` (a short English reason, only when `ok` is false). Extra keys are ignored. `wgpu` may be left out when the game did not
  test it; the launcher then tries wgpu anyway if Vulkan failed. Other output (warnings, validation layer text) is fine before the
  line; the launcher reads the **last** line that starts with `{` and parses.
- **Exit code** 0 when the line says at least one backend works, 1 when none does, **2 for an unknown flag or a usage error**. The
  launcher takes the JSON line first and only uses the code when there is no valid line.
- **Contain the text `probe-gpu`** in the executable (any argument parser does). The launcher scans the binary for it and never
  starts a game that lacks it, so an older game that would ignore the flag does not open its window.
- Catch what it can (a failed device creation is `ok: false` with an `error`, not a crash). If a driver kills the process anyway, the
  launcher counts Vulkan as failed for this GPU and starts with wgpu.

What the launcher does with the answers it cannot use:

| The probe… | The launcher… |
|---|---|
| prints a valid line | decides as above |
| has no `probe-gpu` in the binary, exits 0/1/2 without a valid line, prints garbage | the game has no probe: starts with `auto`; remembered for this game version and GPU |
| does not exit within 8 s | killed; same as no probe, but remembered for one day only |
| dies by a signal or a crash code without a line | Vulkan failed here, wgpu is used; remembered |

## The cache

`<data>/backend.json` (see [design.md](design.md#data-on-disk)) holds the last probe results, keyed by **game version** and a
**GPU key**: a hash of what the system lists without starting a graphics API (Linux: the PCI ids and kernel drivers of the cards in
`/sys/class/drm`, the NVIDIA kernel module version, the age of the Vulkan driver manifests; Windows: the display adapters and driver
versions from the registry). A new game version, a new card or a driver update gives a new key and a new probe. A result is
trusted for 30 days (1 day when the GPU key is unknown or the probe timed out). Changing Settings → Graphics forgets the cache.

## Crash memory

When the launcher chose Vulkan (Auto, the probe said it works) and the game then ends with a crash (a non-zero exit code or a new
report in `crash/`) **within 10 seconds**, the launcher remembers "Vulkan died" for this game version and GPU, shows one
note, and uses wgpu for that version and GPU from then on, without probing. Not counted: a clean exit (code 0), a game the user
stopped, a late crash, a forced setting, a game without a probe (the backend is unknown), and a wgpu start that dies. A new game version,
another GPU or driver, or any change of the Graphics setting gives Vulkan a new chance.

## Diagnostics

About → Diagnostics shows the game version, the Graphics setting, the backend of the last start, why, and the adapter and driver
the probe reported. `<data>/backend.json` has the same in `last`. The launcher never writes the game's `settings.json`
(including its relay `backend_url`) and never passes it in the environment.

## Tests

`crates/core/src/backend/tests.rs` runs the choice against a fake game (a shell script, unix only): probe ok, Vulkan fails, both
fail, no probe (no flag in the binary, exit code 2), a probe that hangs, a probe killed by a signal, the cache per version and GPU
and its expiry, a forced setting, and a Vulkan start that dies at once and the wgpu start after it.

## The player name

The launcher passes `--name <name>` when it has a name: the one typed in Settings, else the OS account's display name,
in the form the game accepts (3 to 16 ASCII letters, digits or `_`). The rules, the per-platform reading of the OS name and
the privacy notes are in [player-name.md](player-name.md). The game reads `--name` for a join; the start screen and a
world ignore it, so the name only reaches the game's own default if the game learns to use it there.
