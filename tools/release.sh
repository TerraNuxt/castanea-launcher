#!/bin/bash
# Builds the launcher releases: tools/release.sh <linux-x86_64|windows-x86_64|all> <version> [--channel stable|beta]
#                                                [--out DIR] [--allow-dirty] [--keep]
#   all       both builds, the only two there are: Linux x86_64 and Windows x86_64
#   version   the workspace version in Cargo.toml (the running launcher compares CARGO_PKG_VERSION with the release)
#   --channel stable (no prerelease part in the version) or beta (needs one); default from the version
#   --out     where the zips go (default ./dist)
#   --keep    leave the task dir and its target/ on the build host (default: removed when done)
# Output in DIR: castanea-launcher-<version>-<platform>.zip (executable at the root, then LICENSE, packed
# deterministically), the same name + .sha256, and a .json. Nothing is signed or published here: see tools/publish.sh.
# Both builds run in the task container on the build host (tools/lib/xenoo.sh, scratch under
# /mnt/bigdrive/castanea-build/scratch/launcher-<id>, deleted at the end); the Windows one is a mingw cross build.
# The host is the user's main PC: everything runs niced and no window is ever opened.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
die() { echo "release.sh: $*" >&2; exit 2; }
usage() { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }

[ $# -ge 2 ] || usage
what=$1
version=$2
shift 2
channel=
out=$root/dist
keep=0
dirty=0
while [ $# -gt 0 ]; do
    case $1 in
        --channel) channel=${2:?--channel needs stable or beta}; shift ;;
        --out) out=${2:?--out needs a folder}; shift ;;
        --keep) keep=1 ;;
        --allow-dirty) dirty=1 ;;
        *) die "unknown argument $1" ;;
    esac
    shift
done
case $what in
    all) platforms="linux-x86_64 windows-x86_64" ;;
    linux-x86_64 | windows-x86_64) platforms=$what ;;
    *) die "build linux-x86_64, windows-x86_64 or all" ;;
esac
if [ "$dirty" = 0 ] && [ -n "$(git -c safe.directory='*' -C "$root" status --porcelain --ignore-submodules 2>/dev/null | grep -v '^??' || true)" ]; then
    die "the work tree has uncommitted changes: commit them (a release is built from a commit), or pass --allow-dirty for a test build"
fi

. "$root/tools/lib/xenoo.sh"
xenoo_init "$root"
mkdir -p "$out"
work=$(mktemp -d)
cleanup() {
    rm -rf "$work"
    if [ "$keep" = 0 ]; then xenoo_clean || echo "release.sh: could not remove the task dir $XTASK on $XHOST" >&2; fi
}
trap cleanup EXIT

echo "release.sh: syncing the tree to $XHOST:$XTASK" >&2
xenoo_sync
xenoo_image
for platform in $platforms; do
    echo "release.sh: building $platform $version" >&2
    xenoo_run 'rm -rf /target/out && tools/build_binary.sh "$1" /target/out' "$platform"
    exe=castanea-launcher
    [ "$platform" = windows-x86_64 ] && exe=castanea-launcher.exe
    mkdir -p "$work/$platform"
    scp -q "$XHOST:$XTASK/target/out/$exe" "$work/$platform/$exe"
    chan=()
    [ -z "$channel" ] || chan=(--channel "$channel")
    "$root/tools/pack_release.sh" "$platform" "$version" "$work/$platform/$exe" "$out" "${chan[@]}"
done
echo "release.sh: next, dry run per zip: tools/publish.sh $out/castanea-launcher-$version-<platform>.zip --channel <stable|beta> --notes-file NOTES.md" >&2
