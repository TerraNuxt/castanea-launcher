# Packaging (notes, not built yet)

The launcher is one binary (`castanea-launcher`, about 23 MB stripped with wgpu on Linux) plus nothing else: strings
are compiled in, all state lives in the per-user folders ([design.md](design.md#data-on-disk)). CI uploads the raw
binaries for Linux and Windows as artifacts.

## Windows

- **Plain zip** first: `castanea-launcher.exe` and `LICENSE`. Release builds use the GUI subsystem (no console).
- **MSI** with [`cargo-wix`](https://github.com/volks73/cargo-wix): per-user install to `%LOCALAPPDATA%\Programs`,
  a Start menu shortcut, no admin rights. Add `wix/main.wxs` with the product GUID fixed once.
- **Code signing** (Authenticode) avoids SmartScreen warnings; needs a certificate (decision for the user).
- An icon: embed with `winres` (`build.rs`) once the game's icon (`assets/interface/icon` in the game) is chosen.
- Portable mode: ship a `castanea-launcher.portable` marker later, or document `CASTANEA_LAUNCHER_HOME`.

## Linux

- **Tarball**: the binary, `LICENSE`, a `.desktop` file (`Exec=castanea-launcher`, `Icon=castanea-launcher`) and the
  hicolor icons, as the game's `tools/desktop` does.
- **AppImage** with `appimagetool` or `cargo-appimage`: the binary plus the desktop file and icon. The Vulkan or GL
  driver and the X11/Wayland libraries come from the host, which is what AppImage expects.
- **Flatpak**: `org.freedesktop.Platform` runtime with the GL extension; permissions `--socket=wayland`,
  `--socket=fallback-x11`, `--device=dri`, `--share=network`, `--filesystem=xdg-data/castanea-launcher:create`.
  File dialogs go through the XDG portal already (`rfd` uses it), so no home access is needed. Starting the game
  binary from inside the sandbox works only for builds inside the sandbox's data folder, so a downloaded build is fine
  and a checkout outside needs `--filesystem` for it.
- **Nix**: a flake output next to the game's, wrapped with the same `LD_LIBRARY_PATH` as `tools/shell.nix`.

## Releases of the launcher

A tag `launcher-vX.Y.Z` could build the archives in CI and attach them with a `SHA256SUMS`. Self-update is not
planned (open decision in [design.md](design.md)).
