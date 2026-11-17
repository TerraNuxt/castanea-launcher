#!/bin/sh
# Syncs this tree to xenoo and runs one command there, niced: tools/remote.sh cargo test --workspace
# The tree lands in ~/castanea-launcher-build (target/ stays there between runs).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
HOST=${LAUNCHER_REMOTE:-xenoo}
rsync -a --delete --exclude /target --exclude /.git --exclude '/docs/screenshots/*.tmp' "$ROOT/" "$HOST:castanea-launcher-build/"
quoted=
for arg in "$@"; do
    quoted="$quoted '$(printf '%s' "$arg" | sed "s/'/'\\\\''/g")'"
done
exec ssh -o BatchMode=yes "$HOST" "cd ~/castanea-launcher-build && nice -n 10 $quoted"
