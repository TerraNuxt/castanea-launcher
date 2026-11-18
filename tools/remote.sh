#!/bin/bash
# Syncs this tree to the build host and runs one command in its task container: tools/remote.sh cargo test --workspace
#   tools/remote.sh --clean     removes the task's container and dir on the host (do it when you are done)
# The tree lands in /mnt/bigdrive/castanea-build/scratch/launcher-<id>/src, target/ stays there between runs
# (see tools/lib/xenoo.sh). No window ever opens on the host; the command runs niced.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/lib/xenoo.sh"
xenoo_init "$root"
if [ "${1:-}" = --clean ]; then xenoo_clean; exit; fi
[ $# -gt 0 ] || { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }
xenoo_sync
xenoo_image
xenoo_run 'exec "$@"' "$@"
