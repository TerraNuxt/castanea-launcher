#!/usr/bin/env bash
# Takes the docs/screenshots PNGs on a Linux host: a demo launcher home with a fake installed game, a private Xvfb
# display, software Vulkan (lavapipe). No window reaches the real desktop. Run from the repo root inside tools/shell.nix:
#   nix-shell tools/shell.nix --run "tools/screenshots.sh target/release/castanea-launcher docs/screenshots"
# Another window size: CASTANEA_SHOT_SIZE=1920x1080 SCREENSHOT_SCREEN=1920x1080x24 (the Xvfb screen must be as big).
# The demo home shows made-up news; a size below the minimum window shows the narrow layout.
set -euo pipefail
BIN=$(realpath "$1")
OUT=$(realpath -m "$2")
DISPLAY_NUMBER=${SCREENSHOT_DISPLAY:-91}
ROOT=$(pwd)
DEMO=$(mktemp -d /tmp/launcher-shots.XXXXXX)
XVFB_PID=

cleanup() {
    if [ -n "$XVFB_PID" ]; then kill "$XVFB_PID" 2>/dev/null || true; fi
    rm -rf "$DEMO"
}
trap cleanup EXIT

# The launcher's home with the fake game installed as the current release, three mods and the example index.
HOME_DIR=$DEMO/home
GAME=$HOME_DIR/data/build/current
mkdir -p "$GAME/assets/engine"
cat > "$GAME/install.json" <<'EOF'
{ "version": "0.2.0", "tag": "v0.2.0", "asset": "castanea-0.2.0-x86_64-unknown-linux-gnu.zip", "sha256": null,
  "notes": "• Smelteries and casting tables\n• The first steam machines\n• Faster world loading\n• Fewer crashes with mods\n• Copper pipes keep their colour" }
EOF
cat > "$GAME/castanea" <<'EOF'
#!/bin/sh
echo "castanea 0.2.0 starting in $(pwd)"
echo "renderer: ${CASTANEA_BACKEND:-auto} (llvmpipe)"
echo "mods: amber 0.1.0, redmoon 0.1.0"
echo "loading assets: 1843 files, 12 layers"
echo "world: new seed 0x5eed, spawn at 12 71 -40"
echo "server tick 20 Hz, 8 worker threads"
echo "warning: no audio device, sound is off" >&2
echo "world loaded, 54 chunks in 0.8 s"
exec sleep 60
EOF
chmod +x "$GAME/castanea"
MODS=$HOME_DIR/data/game/mods
mkdir -p "$MODS/.disabled" "$HOME_DIR/config"
for id in amber redmoon; do
    mkdir -p "$MODS/$id"
    cp "$ROOT/crates/core/tests/fixtures/game-mods/$id/mod.json" "$MODS/$id/"
done
mkdir -p "$MODS/.disabled/wasm_hello"
cp "$ROOT/crates/core/tests/fixtures/game-mods/wasm_hello/mod.json" "$MODS/.disabled/wasm_hello/"
cat > "$HOME_DIR/config/launcher.json" <<EOF
{ "language": "en", "offline": false, "indexes": ["$ROOT/docs/examples/index.json"],
  "game": { "player_name": "Anna", "graphics": "vulkan" } }
EOF

# A display of our own: nothing reaches the real desktop. The shots are taken from inside the launcher.
Xvfb ":$DISPLAY_NUMBER" -screen 0 "${SCREENSHOT_SCREEN:-1280x800x24}" -nolisten tcp >/dev/null 2>&1 &
XVFB_PID=$!
sleep 1
mkdir -p "$OUT"
env -u WAYLAND_DISPLAY DISPLAY=":$DISPLAY_NUMBER" \
    HOME="$DEMO" CASTANEA_LAUNCHER_HOME="$HOME_DIR" \
    VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json WGPU_BACKEND=vulkan \
    LANG=en_US.UTF-8 \
    "$BIN" --screenshots "$OUT"
ls -l "$OUT"
