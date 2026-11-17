#!/bin/bash
# Builds one launcher release zip: tools/release.sh <platform> <version> [--channel stable|beta] [--out DIR]
#   platform  linux-x86_64 (native release build on the build host) or windows-x86_64 (cross build for
#             x86_64-pc-windows-gnu in a throwaway container there, like tools/winck.sh)
#   version   the workspace version in Cargo.toml (the running launcher compares CARGO_PKG_VERSION with the release)
#   --channel stable (no prerelease part in the version) or beta (needs one); default from the version
# Output in DIR (default ./dist): castanea-launcher-<version>-<platform>.zip (the executable at the root, then LICENSE,
# packed deterministically) and the same name + .sha256. Nothing is signed or published here: see tools/publish.sh.
# The build host is xenoo, the user's main PC: everything runs niced and no window is ever opened.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
die() { echo "release.sh: $*" >&2; exit 2; }
usage() { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }

[ $# -ge 2 ] || usage
platform=$1
version=$2
shift 2
channel=
out=$root/dist
while [ $# -gt 0 ]; do
    case $1 in
        --channel) channel=${2:?--channel needs stable or beta}; shift ;;
        --out) out=${2:?--out needs a folder}; shift ;;
        *) die "unknown argument $1" ;;
    esac
    shift
done
case $platform in linux-x86_64 | windows-x86_64) ;; *) die "platform must be linux-x86_64 or windows-x86_64" ;; esac
re='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
[[ $version =~ $re ]] || die "$version is not a semver version without build metadata"
if [[ $version == *-* ]]; then want=beta; else want=stable; fi
channel=${channel:-$want}
[ "$channel" = "$want" ] || die "a $channel release must $([ "$channel" = beta ] && echo 'have' || echo 'not have') a prerelease part, but the version is $version"
declared=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' "$root/Cargo.toml")
[ "$declared" = "$version" ] || die "Cargo.toml says version $declared, not $version: bump it first (and commit), so the launcher knows its own version"

host=${LAUNCHER_REMOTE:-xenoo}
build='castanea-launcher-build'
name=castanea-launcher-$version-$platform
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "release.sh: syncing the tree to $host and building $platform $version" >&2
rsync -a --delete --exclude /target --exclude /.git --exclude /.winck --exclude /dist "$root/" "$host:$build/"
ssh -o BatchMode=yes "$host" "mkdir -p $build/.winck/target $build/.winck/cargo-home"

if [ "$platform" = linux-x86_64 ]; then
    ssh -o BatchMode=yes "$host" "cd ~/$build && nice -n 10 cargo build --release --locked -p castanea-launcher"
    # --version prints and exits before any window opens.
    said=$(ssh -o BatchMode=yes "$host" "cd ~/$build && target/release/castanea-launcher --version")
    [ "$said" = "castanea-launcher $version" ] || die "the binary says '$said', expected 'castanea-launcher $version'"
    scp -q "$host:$build/target/release/castanea-launcher" "$work/castanea-launcher"
else
    image=${WINCK_IMAGE:-castanea-builder:nextest}
    script='set -e
rustup target add x86_64-pc-windows-gnu >/dev/null
apt-get update -qq >/dev/null
apt-get install -y -qq gcc-mingw-w64-x86-64 >/dev/null
cargo build --release --locked -p castanea-launcher --target x86_64-pc-windows-gnu'
    ssh -o BatchMode=yes "$host" "B=\$HOME/$build; nice -n 10 docker run --rm --init --cpus 6 --memory 8g -w /work -v \$B:/work \
        -v \$B/.winck/target:/target -v \$B/.winck/cargo-home:/cargo-home -e CARGO_TARGET_DIR=/target -e CARGO_HOME=/cargo-home \
        $image bash -c '$script'"
    scp -q "$host:$build/.winck/target/x86_64-pc-windows-gnu/release/castanea-launcher.exe" "$work/castanea-launcher.exe"
fi

exe=$(ls "$work"/castanea-launcher*)
python3 "$root/tools/pack_launcher.py" "$exe" "$out/$name.zip" "$root/LICENSE"
(cd "$out" && sha256sum "$name.zip" > "$name.zip.sha256")
printf '{"product":"launcher","version":"%s","platform":"%s","channel":"%s"}\n' "$version" "$platform" "$channel" > "$out/$name.json"
echo "release.sh: $out/$name.zip ($(du -h "$out/$name.zip" | cut -f1)), $(cut -d' ' -f1 "$out/$name.zip.sha256")" >&2
echo "release.sh: next, dry run: tools/publish.sh $out/$name.zip --channel $channel --notes-file NOTES.md" >&2
