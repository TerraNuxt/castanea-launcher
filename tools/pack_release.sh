#!/bin/bash
# Packs one launcher release: tools/pack_release.sh <platform> <version> <executable> <outdir> [--channel stable|beta]
# Writes castanea-launcher-<version>-<platform>.zip (the executable at the root, then LICENSE, deterministic: tools/pack_launcher.py),
# the same name + .sha256, and a small .json with product, version, platform and channel (what tools/publish.sh reads).
# Shared by tools/release.sh and the GitHub workflow. Nothing is signed or published here.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
die() { echo "pack_release.sh: $*" >&2; exit 2; }
[ $# -ge 4 ] || { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }
platform=$1 version=$2 exe=$3 out=$4
shift 4
channel=
while [ $# -gt 0 ]; do
    case $1 in
        --channel) channel=${2:?--channel needs stable or beta}; shift ;;
        *) die "unknown argument $1" ;;
    esac
    shift
done
case $platform in
    linux-x86_64) want_name=castanea-launcher ;;
    windows-x86_64) want_name=castanea-launcher.exe ;;
    *) die "platform must be linux-x86_64 or windows-x86_64" ;;
esac
[ "$(basename "$exe")" = "$want_name" ] || die "a $platform release packs $want_name, not $(basename "$exe")"
[ -f "$exe" ] || die "$exe is missing"
re='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
[[ $version =~ $re ]] || die "$version is not a semver version without build metadata"
if [[ $version == *-* ]]; then want=beta; else want=stable; fi
channel=${channel:-$want}
[ "$channel" = "$want" ] || die "a $channel release must $([ "$channel" = beta ] && echo 'have' || echo 'not have') a prerelease part, but the version is $version"
declared=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' "$root/Cargo.toml")
[ "$declared" = "$version" ] || die "Cargo.toml says version $declared, not $version: bump it first (and commit), so the launcher knows its own version"

mkdir -p "$out"
name=castanea-launcher-$version-$platform
python3 "$root/tools/pack_launcher.py" "$exe" "$out/$name.zip" "$root/LICENSE"
(cd "$out" && sha256sum "$name.zip" >"$name.zip.sha256")
printf '{"product":"launcher","version":"%s","platform":"%s","channel":"%s"}\n' "$version" "$platform" "$channel" >"$out/$name.json"
echo "pack_release.sh: $out/$name.zip ($(du -h "$out/$name.zip" | cut -f1)) $(cut -d' ' -f1 "$out/$name.zip.sha256")" >&2
