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
- **Nix**: the flake, see [nix.md](nix.md).

## Releases of the launcher

Launchers update themselves from the release server ([design.md](design.md#launcher-self-update)). A launcher release
is a zip with `castanea-launcher` (`castanea-launcher.exe`) at its root and a `LICENSE`, signed with the release key and
published as `product=launcher`. CI builds the binaries as artifacts but **never signs or publishes**: the private
key lives only on the build host (xenoo), in `~/.castanea-signing/release-signing.key` (mode 0600, back it up offline),
with `release-signing.pub` next to it. Its public half is in `crates/core/src/trust.rs` (`key_id`
`c6dafdb0e8d6c8ff`) and in the backend's `docs/releases.md`. Publishing is the user's go-ahead, like a game release.

### Release steps

Run from the launcher checkout on the dev machine (WSL). `tools/release.sh` and `tools/publish.sh` use ssh to the build
host (`LAUNCHER_REMOTE`, default `xenoo`), niced; no window opens there.

1. **Version.** Set `version` under `[workspace.package]` in `Cargo.toml` to the new version (semver, a prerelease
   such as `0.3.0-beta.1` for the beta channel), run the tests, commit and push. The running launcher compares its own
   `CARGO_PKG_VERSION` with the release, so `release.sh` refuses a version that is not the one in `Cargo.toml`.
2. **Build and pack**, once per platform:

   ```sh
   tools/release.sh linux-x86_64 0.3.0            # native release build on xenoo, checks `--version`
   tools/release.sh windows-x86_64 0.3.0          # cross build for x86_64-pc-windows-gnu in a container there
   ```

   Output in `dist/`: `castanea-launcher-0.3.0-<platform>.zip` (the executable at the root, then `LICENSE`, packed
   deterministically by `tools/pack_launcher.py`: fixed times, order and modes, so the same binary gives the same
   SHA-256), the same name plus `.sha256`, and a small `.json` with product, version, platform and channel.
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

### Key rotation

Create a new pair on the build host (`castanea-backend keygen --out DIR`), add its public key to
`/etc/castanea-backend/release-public-keys` on the server, add it to `TRUST_KEYS` in `crates/core/src/trust.rs` **next to
the old one**, release a launcher that carries both, and only then sign with the new key. Drop the old key from the list
in a later launcher once no release needs it. A launcher that has only the old key cannot verify releases signed by the new
one, so a key must be in a released launcher before anything is signed with it.

### Not yet

An MSI (`cargo-wix`) and Authenticode signing are still open (see above). A package-managed or AppImage launcher is
never replaced by the launcher itself; it shows the new version and the download page.
