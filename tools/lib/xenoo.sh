#!/bin/bash
# Shared by tools/remote.sh, tools/winck.sh and tools/release.sh: one task on the build host (xenoo), the way
# castanea's tools/cargo-remote does it. Source it, then call xenoo_init <repo root>.
#   task id        launcher-<12 hex of the sha256 of the repo path>
#   task dir       $BASE/scratch/<id>/{src,target}, on the big disk (BASE = $CASTANEA_BUILD_BASE, default
#                  /mnt/bigdrive/castanea-build); nothing is ever written to the host's home
#   container      castanea-task-<id>, labelled castanea.task=<id>, removed when the command ends
#   shared         $BASE/cargo-home (the crate cache, as for the game)
#   image          castanea-launcher-builder:<hash of tools/builder/Dockerfile>, built on first use
# xenoo_clean removes the container and the task dir; release.sh and winck.sh call it when they are done.

xenoo_init() {
    XROOT=$1
    XHOST=${LAUNCHER_REMOTE:-${CASTANEA_BUILD_HOST:-xenoo}}
    XBASE=${CASTANEA_BUILD_BASE:-/mnt/bigdrive/castanea-build}
    XID=launcher-$(printf '%s' "$XROOT" | sha256sum | cut -c1-12)
    XTASK=$XBASE/scratch/$XID
    XDOCKERFILE=$XROOT/tools/builder/Dockerfile
    XIMAGE=castanea-launcher-builder:$(sha256sum "$XDOCKERFILE" | cut -c1-12)
}

xenoo_ssh() { ssh -o BatchMode=yes "$XHOST" "$@"; }

# Copies the work tree (not target, .git, dist) to the task dir.
xenoo_sync() {
    xenoo_ssh "mkdir -p $XTASK/src $XTASK/target $XBASE/cargo-home"
    rsync -a --delete --exclude /target --exclude /.git --exclude /dist --exclude '/docs/screenshots/*.tmp' \
        "$XROOT/" "$XHOST:$XTASK/src/"
}

# Builds the image when this Dockerfile has none yet (minutes the first time, then cached).
xenoo_image() {
    if xenoo_ssh "docker image inspect $XIMAGE >/dev/null 2>&1"; then return 0; fi
    echo "xenoo: building $XIMAGE on $XHOST (once)" >&2
    xenoo_ssh "nice -n 10 docker build -t $XIMAGE -" <"$XDOCKERFILE" >&2
}

# xenoo_run <script> [args...]: runs the script with bash in the task container, in /work (the synced tree).
xenoo_run() {
    local script=$1 quoted= arg
    shift
    for arg in "$@"; do quoted="$quoted '$(printf '%s' "$arg" | sed "s/'/'\\\\''/g")'"; done
    xenoo_ssh "nice -n 10 docker run --rm --init --name castanea-task-$XID --label castanea.task=$XID --cpus 6 --memory 8g \
        -w /work -v $XTASK/src:/work -v $XTASK/target:/target -v $XBASE/cargo-home:/cargo-home \
        -e CARGO_TARGET_DIR=/target -e CARGO_HOME=/cargo-home -e CARGO_TERM_COLOR=never \
        $XIMAGE bash -c '$(printf '%s' "$script" | sed "s/'/'\\\\''/g")' _ $quoted"
}

# Removes the container and the task dir; the path is checked first, as it is built from variables.
xenoo_clean() {
    case $XID in launcher-????????????) ;; *) return 1 ;; esac
    case $XBASE in /?*) ;; *) return 1 ;; esac
    [ "$XTASK" = "$XBASE/scratch/$XID" ] || return 1
    xenoo_ssh "docker rm -f castanea-task-$XID >/dev/null 2>&1; rm -rf $XTASK"
}
