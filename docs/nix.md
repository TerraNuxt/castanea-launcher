# Nix

`flake.nix` builds the launcher for `x86_64-linux` and `aarch64-linux`:

| Output | What |
|---|---|
| `packages.default` | `bin/castanea-launcher`, `share/applications/castanea-launcher.desktop` and the 512 px icon. The wrapper has the Vulkan, GL, Wayland, X11, xkbcommon, alsa and udev libraries on `LD_LIBRARY_PATH`, for the launcher and the game. |
| `apps.default` | `nix run` starts the launcher (it opens a window). |
| `devShells.default` | cargo, rustc, clippy, rustfmt, rust-analyzer and the same libraries for `cargo run`. |
| `overlays.default` | adds `castanea-launcher` to `pkgs`. |

## The game's libraries on NixOS

The wrapper of `castanea-launcher` puts the libraries of the launcher window and of the game on `LD_LIBRARY_PATH`:
alsa-lib (the game links `libasound.so.2`), vulkan-loader, libxkbcommon, wayland, libGL, libX11, libxcb, libXcursor,
libXi, libXrandr and libudev (systemd). The game is started as a child and inherits that environment, so it runs
without `nix-ld` or an FHS shell. When `/run/opengl-driver/lib` exists, the wrapper appends it (the GPU driver), and
`/run/opengl-driver/share` goes to `XDG_DATA_DIRS` so the Vulkan loader finds the ICDs in `.../vulkan/icd.d`. Other
systems are not affected: the folders are added only if they exist.

If the game still reports `error while loading shared libraries: <lib>`, the launcher says which library is missing
in Game output and on the Play page. Start the launcher through the flake's package, not a copy of the unwrapped binary.

Build without installing: `nix build` (the result is in `./result`). The sandbox build skips the tests; CI runs them.

## Install on NixOS

Add the flake as an input and install the package, as a flake module:

```nix
# flake.nix
inputs.castanea-launcher.url = "github:TerraNuxt/castanea-launcher";

# in the system (or home) configuration
environment.systemPackages = [ inputs.castanea-launcher.packages.${pkgs.system}.default ];
```

Or through the overlay: `nixpkgs.overlays = [ inputs.castanea-launcher.overlays.default ];` and then
`environment.systemPackages = [ pkgs.castanea-launcher ];`.

## Updates

The flake lock pins the launcher to one commit. To move to the newest `main`:

```sh
nix flake update castanea-launcher     # in the configuration's folder
sudo nixos-rebuild switch --flake .#<host>
```

## Game builds

The game repo is private, so the launcher does not read GitHub: it gets the game from the release server
(`https://castanea.xenoo.dev`, see [release-api.md](release-api.md)). No token is needed; Settings → Advanced → Release
server or `CASTANEA_RELEASE_URL` point it somewhere else.
