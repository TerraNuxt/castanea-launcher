#!/usr/bin/env bash
# Takes the docs/screenshots PNGs on a Linux host: a demo launcher home, a fake game, a private Xvfb display,
# software Vulkan (lavapipe). No window reaches the real desktop. Run from the repo root inside tools/shell.nix:
#   nix-shell tools/shell.nix --run "tools/screenshots.sh target/release/castanea-launcher docs/screenshots"
set -euo pipefail
BIN=$(realpath "$1")
OUT=$(realpath -m "$2")
DISPLAY_NUMBER=${SCREENSHOT_DISPLAY:-92}
ROOT=$(pwd)
DEMO=$(mktemp -d /tmp/launcher-shots.XXXXXX)
XVFB_PID=

cleanup() {
    if [ -n "$XVFB_PID" ]; then kill "$XVFB_PID" 2>/dev/null || true; fi
    rm -rf "$DEMO"
}
trap cleanup EXIT

# A fake game build: assets/ and a binary that prints a plausible start log.
GAME=$DEMO/castanea-0.2.0
mkdir -p "$GAME/assets/engine"
echo "0.2.0" > "$GAME/version.txt"
cat > "$GAME/castanea" <<'EOF'
#!/bin/sh
echo "castanea 0.2.0 starting in $(pwd)"
echo "renderer: ${CASTANEA_BACKEND:-auto} (llvmpipe)"
echo "mods: amber 0.1.0, redmoon 0.1.0"
echo "loading assets: 1843 files, 12 layers"
echo "world: new seed 0x5eed, spawn at 12 71 -40"
echo "server tick 20 Hz, 8 worker threads"
echo "warning: no audio device, sound is off" >&2
echo "saved world, bye"
EOF
chmod +x "$GAME/castanea"

# The launcher's home with one profile, three mods and the example index.
HOME_DIR=$DEMO/home
MODS=$HOME_DIR/data/instances/default/mods
mkdir -p "$MODS/.disabled" "$HOME_DIR/config"
for id in amber redmoon; do
    mkdir -p "$MODS/$id"
    cp "$ROOT/crates/core/tests/fixtures/game-mods/$id/mod.json" "$MODS/$id/"
done
mkdir -p "$MODS/.disabled/wasm_hello"
cp "$ROOT/crates/core/tests/fixtures/game-mods/wasm_hello/mod.json" "$MODS/.disabled/wasm_hello/"
cat > "$HOME_DIR/config/launcher.json" <<EOF
{ "language": "en", "offline": false, "sources": [], "indexes": ["$ROOT/docs/examples/index.json"] }
EOF
cat > "$HOME_DIR/config/profiles.json" <<'EOF'
{ "profiles": [
  { "id": "default", "name": "Survival", "start": { "kind": "world", "name": "lake", "seed": "0x5eed", "host_port": null },
    "player_name": "Anna", "graphics": "vulkan" },
  { "id": "creative", "name": "Creative test", "extra_args": "--gamemode creative" }
] }
EOF

Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 &
XVFB_PID=$!
sleep 1
mkdir -p "$OUT"
env -u WAYLAND_DISPLAY DISPLAY=":$DISPLAY_NUMBER" \
    HOME="$DEMO" CASTANEA_LAUNCHER_HOME="$HOME_DIR" CASTANEA_GAME_DIR="$GAME" \
    VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json WGPU_BACKEND=vulkan \
    LANG=en_US.UTF-8 \
    "$BIN" --screenshots "$OUT"
ls -l "$OUT"
