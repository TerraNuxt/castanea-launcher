# Packaging

The launcher is one binary (`castanea-launcher`, about 23 MB stripped with wgpu on Linux) plus nothing else: strings
are compiled in, all state lives in the per-user folders ([design.md](design.md#data-on-disk)).

## The two builds

There are exactly two launcher builds, and nothing else is released:

| Platform (release API) | File | Built as |
|---|---|---|
| `linux-x86_64` | `castanea-launcher-<version>-linux-x86_64.zip`: `castanea-launcher` (x86_64 ELF), `LICENSE` | native release build in the build host's container (Debian 12, glibc 2.36) |
| `windows-x86_64` | `castanea-launcher-<version>-windows-x86_64.zip`: `castanea-launcher.exe` (PE32+, GUI subsystem), `LICENSE` | cross build for `x86_64-pc-windows-gnu` in the same container (mingw-w64, posix threads) |

Each zip comes with `<name>.zip.sha256` and `<name>.json` (product, version, platform, channel) next to it. The zip is packed
deterministically (`tools/pack_launcher.py`: fixed times, order and modes), so the same executable gives the same SHA-256.
The Windows exe has the launcher's icon (`packaging/windows/castanea-launcher.ico`, made from `packaging/linux/castanea-launcher.png`
by `tools/make_ico.py`) and version info (company, description, file and product version from `Cargo.toml`), compiled into it by
`crates/ui/build.rs` with `windres` (no extra crate). Without `windres` or `rc` the build still works and warns that the exe has
no icon.

Where each is built:

- **On the build host (xenoo)**, `tools/release.sh`: the release you sign and publish. The task container, its `target/` and the
  synced tree live under `/mnt/bigdrive/castanea-build/scratch/launcher-<id>` and are deleted when the script ends
  (`--keep` leaves them). The image `castanea-launcher-builder:<hash of tools/builder/Dockerfile>` (the game's builder image with
  mingw-w64, cargo-deny and zip) is built once, so a release installs no packages.
- **On GitHub**, `.github/workflows/release-build.yml` (`workflow_dispatch` and pushed `v*` tags): runs fmt, clippy, the tests
  (also the release-mode subset) and `cargo deny`, then builds both zips with the same scripts (`tools/build_binary.sh`,
  `tools/pack_release.sh`) and uploads them as workflow artifacts. **It never signs or publishes.** Its zips are for testing
  and as a second opinion: the release you publish is the one from the build host, because signing happens there. CI uses the
  same Rust version (pinned in the workflow) and Ubuntu 22.04 (glibc 2.35), but its bytes are not promised to equal the build host's.

## Windows

- **Plain zip**: `castanea-launcher.exe` and `LICENSE`. Release builds use the GUI subsystem (no console).
- **MSI** with [`cargo-wix`](https://github.com/volks73/cargo-wix) is not built (see below): per-user install to
  `%LOCALAPPDATA%\Programs`, a Start menu shortcut, no admin rights.
- **Code signing** (Authenticode) avoids SmartScreen warnings; needs a certificate (decision for the user).
- Portable mode: `CASTANEA_LAUNCHER_HOME` (a marker file may follow).

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
- **Nix**: the flake, see [nix.md](nix.md).

## Releases of the launcher

Launchers update themselves from the release server ([design.md](design.md#launcher-self-update)). A launcher release
is a zip with `castanea-launcher` (`castanea-launcher.exe`) at its root and a `LICENSE`, signed with the release key and
published as `product=launcher`. CI builds the zips as artifacts but **never signs or publishes**: the private
key lives only on the build host (xenoo), in `~/.castanea-signing/release-signing.key` (mode 0600, back it up offline),
with `release-signing.pub` next to it. Its public half is in `crates/core/src/trust.rs` (`key_id`
`c6dafdb0e8d6c8ff`) and in the backend's `docs/releases.md`. Publishing is the user's go-ahead, like a game release.

### Release steps

Run from the launcher checkout on the dev machine (WSL). `tools/release.sh` and `tools/publish.sh` use ssh to the build
host (`LAUNCHER_REMOTE`, default `xenoo`), niced; no window opens there.

1. **Version.** Set `version` under `[workspace.package]` in `Cargo.toml` to the new version (semver, a prerelease
   such as `0.3.0-beta.1` for the beta channel), run the tests, **commit** and push. The running launcher compares its own
   `CARGO_PKG_VERSION` with the release, so the scripts refuse a version that is not the one in `Cargo.toml`, and a release is
   built from a commit (`--allow-dirty` is for test builds).
2. **Build and pack both** (about ten minutes the first time, less with the host's crate cache):

   ```sh
   tools/release.sh all 0.3.0         # both zips into ./dist; or one platform: linux-x86_64 | windows-x86_64
   ```

   It syncs the tree to `/mnt/bigdrive/castanea-build/scratch/launcher-<id>/src` on the build host, runs `tools/build_binary.sh`
   for each platform in the task container (`--locked`, source paths mapped to fixed names, no link timestamp; Linux checks
   `--version`; Windows checks the PE: GUI subsystem, icon and version resources, only Windows DLLs), copies the executables back,
   packs them with `tools/pack_release.sh`, and deletes the task dir and container on the host. Output in `dist/`:
   `castanea-launcher-0.3.0-linux-x86_64.zip` and `-windows-x86_64.zip`, each with `.sha256` and `.json`.
   Packing the same executable again gives the same SHA-256 (`tools/pack_release.sh <platform> <version> <exe> <dir>`, then
   `sha256sum`).
3. **Notes** in a markdown file (what changed, a few lines; the launcher shows them as plain text).
4. **Dry run**, per zip: `tools/publish.sh dist/castanea-launcher-0.3.0-linux-x86_64.zip --channel stable --notes-file NOTES.md`.
   It checks the zip against its `.sha256` and its contents, copies it to the build host, signs and verifies it there with
   `castanea-backend sign` / `verify` (the key stays on the host), reads the public API and prints the command it would run.
   Nothing is uploaded or registered.
5. **Publish** (only with the user's go-ahead): the same command with `--yes`. It runs the backend's
   `deploy/publish-release.sh --product launcher` on the build host, which signs
   (`castanea-backend sign --key ~/.castanea-signing/release-signing.key --product launcher --version … --platform …
   --channel … --zip …`), uploads through the jump host to the server's launcher staging dir, registers it with the
   admin CLI (the server verifies the signature against its public keys), compares the SHA-256 the server computed,
   and reads the release back from the public API: same product, hash, size and signature, the signature verifies against
   the key, and a ranged download matches. Do both platforms.
6. **Check as a user**: start an older launcher (`CASTANEA_LAUNCHER_HOME=/tmp/x castanea-launcher`, copy it to a writable
   folder first, and not the one in `dist/`): About says `restart to update to 0.3.0`; restart; About says `up to date`.
   `CASTANEA_RELEASE_URL` points a test launcher at another server.
7. **Roll back** a bad release: delete it on the server
   (`ssh … sudo -u castanea castanea-backend-admin delete --product launcher --version V --platform P`); launchers
   already updated keep it (they have no downgrade), so ship a fixed higher version. A release that does not start is
   rolled back on the user's machine by the launcher itself and skipped.

The GitHub workflow is a second way to get the zips for a test: Actions → release-build → Run workflow (or push a `v<version>` tag
that matches `Cargo.toml`), then download the artifact. Do not publish those; run step 2 on the build host.

Other helpers use the same task layout on the build host: `tools/remote.sh <command>` (for example
`tools/remote.sh cargo test --workspace --locked`; `tools/remote.sh --clean` removes the task) and `tools/winck.sh` (the Windows
cross-check). Scratch is only under `/mnt/bigdrive/castanea-build/scratch/launcher-<id>`, never in the host's home.

### Key rotation

Create a new pair on the build host (`castanea-backend keygen --out DIR`), add its public key to
`/etc/castanea-backend/release-public-keys` on the server, add it to `TRUST_KEYS` in `crates/core/src/trust.rs` **next to
the old one**, release a launcher that carries both, and only then sign with the new key. Drop the old key from the list
in a later launcher once no release needs it. A launcher that has only the old key cannot verify releases signed by the new
one, so a key must be in a released launcher before anything is signed with it.

### Not yet

An MSI (`cargo-wix`) and Authenticode signing are still open (see above). A package-managed or AppImage launcher is
never replaced by the launcher itself; it shows the new version and the download page.
