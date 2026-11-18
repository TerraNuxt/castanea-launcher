#!/bin/bash
# Headless check that a start the way the launcher makes it opens the start screen and no world.
#   tools/check-start-screen.sh <game build dir> [--name N]    (own Xvfb, CASTANEA_OFFSCREEN=1, no Wayland, scratch dir deleted)
# The game opens a world when it gets arguments but no --menu; a world shows up as saves/ in its working directory.
# On NixOS run it through a wrapper that sets LD_LIBRARY_PATH: LAUNCHER_ENV_WRAPPER="bash libenv.sh" tools/check-start-screen.sh ...
set -euo pipefail
build=$(cd "${1:?game build dir}" && pwd)
shift
[ $# -gt 0 ] || set -- --name Checker
args=(--menu "$@")
scratch=$(mktemp -d)
trap 'kill "${xvfb:-0}" 2>/dev/null || true; rm -rf "$scratch"' EXIT
mkdir -p "$scratch/mods"
ln -s "$build/assets" "$scratch/assets"
Xvfb :98 -screen 0 1280x720x24 >/dev/null 2>&1 </dev/null &
xvfb=$!
sleep 2
cd "$scratch"
env -u WAYLAND_DISPLAY DISPLAY=:98 CASTANEA_OFFSCREEN=1 timeout 15 ${LAUNCHER_ENV_WRAPPER:-} "$build/castanea" "${args[@]}" >out.txt 2>&1 </dev/null || true
if [ -e saves ]; then
  echo "FAIL: the game opened a world (saves/ exists) with: ${args[*]}" >&2
  exit 1
fi
grep -q -e "graphics backend" out.txt || { echo "FAIL: the game did not start:" >&2; head -5 out.txt >&2; exit 1; }
echo "PASS: no world opened with: ${args[*]}"
