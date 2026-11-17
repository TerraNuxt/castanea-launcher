# Nix

`flake.nix` builds the launcher for `x86_64-linux` and `aarch64-linux`:

| Output | What |
|---|---|
| `packages.default` | `bin/castanea-launcher`, `share/applications/castanea-launcher.desktop` and the 512 px icon. The binary has the Vulkan, GL, Wayland, X11 and xkbcommon libraries on `LD_LIBRARY_PATH`. |
| `apps.default` | `nix run` starts the launcher (it opens a window). |
| `devShells.default` | cargo, rustc, clippy, rustfmt, rust-analyzer and the same libraries for `cargo run`. |
| `overlays.default` | adds `castanea-launcher` to `pkgs`. |

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

## Private repo

While `TerraNuxt/castanea-launcher` is private, nix needs a GitHub token to fetch it. Give it one as
`access-tokens = github.com=<token>` in `nix.conf` or in `NIX_CONFIG`. The xenoo `switch` script reads it from
`gh auth token` at run time. This is separate from `CASTANEA_GITHUB_TOKEN`, which the launcher uses to download game
builds from a private game repo. Once the repo is public, no token is needed for either.
