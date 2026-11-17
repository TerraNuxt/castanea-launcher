#!/bin/bash
# Signs and publishes a launcher release through the backend's publish script: dry run by default.
# Usage: tools/publish.sh <castanea-launcher-V-P.zip> --channel stable|beta --notes-file FILE [--replace] [--yes]
#   without --yes  dry run: checks the zip, signs and verifies it on the build host (the private key never leaves it),
#                  reads the public API, and prints what would happen. Nothing is uploaded or registered.
#   with --yes     publishes (deploy/publish-release.sh of castanea-backend, run on the build host with --product launcher).
# Settings (environment): LAUNCHER_REMOTE the build host (default xenoo); CASTANEA_BACKEND_REPO the backend checkout on
# it (default ~/castanea-backend); CASTANEA_PUBLIC_URL (default https://castanea.xenoo.dev).
# CI never signs or publishes. Publishing is the user's go-ahead, like a game release (docs/packaging.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
die() { echo "publish.sh: $*" >&2; exit 2; }
zip=${1:-}
[ -f "$zip" ] || { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }
shift
channel= notes= replace= yes=0
while [ $# -gt 0 ]; do
    case $1 in
        --channel) channel=${2:?--channel needs stable or beta}; shift ;;
        --notes-file) notes=${2:?--notes-file needs a file}; shift ;;
        --replace) replace=--replace ;;
        --yes) yes=1 ;;
        *) die "unknown argument $1" ;;
    esac
    shift
done
case $channel in stable | beta) ;; *) die "--channel stable or beta is required" ;; esac
[ -f "$notes" ] || die "--notes-file FILE is required and must exist"

name=$(basename "$zip")
re='^castanea-launcher-(.+)-(linux-x86_64|windows-x86_64)\.zip$'
[[ $name =~ $re ]] || die "$name is not castanea-launcher-<version>-<platform>.zip"
version=${BASH_REMATCH[1]}
platform=${BASH_REMATCH[2]}
[[ $channel == stable && $version == *-* ]] && die "a stable release has no prerelease part, but $version has one"
[[ $channel == beta && $version != *-* ]] && die "a beta release needs a prerelease part, but $version has none"

# The zip must match its .sha256 and hold the executable at its root.
sumfile=$zip.sha256
[ -f "$sumfile" ] || die "$sumfile is missing: build the zip with tools/release.sh"
local_sha=$(sha256sum "$zip" | cut -d' ' -f1)
[ "$(cut -d' ' -f1 "$sumfile")" = "$local_sha" ] || die "$name does not match its .sha256"
exe=castanea-launcher
[ "$platform" = windows-x86_64 ] && exe=castanea-launcher.exe
python3 -I - "$zip" "$exe" <<'PY' || die "the zip must hold $exe at its root and no unsafe path"
import sys, zipfile
names = zipfile.ZipFile(sys.argv[1]).namelist()
bad = [n for n in names if n.startswith("/") or ".." in n.split("/") or "\\" in n or ":" in n]
sys.exit(0 if sys.argv[2] in names and not bad else 1)
PY
echo "publish.sh: $name ok, sha256 $local_sha, $(wc -c < "$zip" | tr -d ' ') bytes" >&2

host=${LAUNCHER_REMOTE:-xenoo}
backend=${CASTANEA_BACKEND_REPO:-castanea-backend}
public=${CASTANEA_PUBLIC_URL:-https://castanea.xenoo.dev}
ssh_opts=(-o BatchMode=yes)
tmp=$(ssh "${ssh_opts[@]}" "$host" 'mktemp -d')
trap 'ssh "${ssh_opts[@]}" "$host" "rm -rf $tmp" >/dev/null 2>&1 || true' EXIT
scp -q "${ssh_opts[@]}" "$zip" "$notes" "$host:$tmp/"
remote_zip=$tmp/$name
remote_notes=$tmp/$(basename "$notes")

if [ $yes -eq 0 ]; then
    echo "publish.sh: DRY RUN, nothing is uploaded or registered" >&2
    ssh "${ssh_opts[@]}" "$host" bash -s -- "$backend" "$remote_zip" "$version" "$platform" "$channel" "$public" <<'REMOTE'
set -euo pipefail
backend=$1 zip=$2 version=$3 platform=$4 channel=$5 public=$6
case $backend in /*) ;; *) backend=$HOME/$backend ;; esac
bin=$backend/target/release/castanea-backend
[ -x "$bin" ] || bin=$(command -v castanea-backend || true)
key=${CASTANEA_SIGNING_KEY:-$HOME/.castanea-signing/release-signing.key}
[ -f "$key" ] || { echo "no signing key at $key" >&2; exit 1; }
if [ -z "$bin" ]; then
    echo "no castanea-backend binary yet (publish-release.sh would build it); cannot sign-check now" >&2
else
    nice -n 10 "$bin" sign --key "$key" --product launcher --version "$version" --platform "$platform" \
        --channel "$channel" --zip "$zip" > "$zip.sig"
    nice -n 10 "$bin" verify --pub "${key%.key}.pub" --product launcher --version "$version" --platform "$platform" \
        --channel "$channel" --zip "$zip" --signature "$(cat "$zip.sig")"
    echo "signed and verified locally: $(cat "$zip.sig")"
fi
echo "public API now lists for launcher $platform:"
curl -fsS "$public/v1/releases?product=launcher&platform=$platform" || echo "(could not read the public API)"
echo
REMOTE
    echo "publish.sh: would run on $host: $backend/deploy/publish-release.sh --product launcher --version $version --platform $platform --channel $channel --notes-file $(basename "$notes") --zip $name $replace" >&2
    echo "publish.sh: when that looks right, run the same command again with --yes" >&2
    exit 0
fi

echo "publish.sh: publishing launcher $version $platform ($channel)" >&2
ssh "${ssh_opts[@]}" "$host" bash -s -- "$backend" "$remote_zip" "$remote_notes" "$version" "$platform" "$channel" "$replace" <<'REMOTE'
set -euo pipefail
backend=$1 zip=$2 notes=$3 version=$4 platform=$5 channel=$6 replace=${7:-}
case $backend in /*) ;; *) backend=$HOME/$backend ;; esac
CASTANEA_BUILD_HOST=local nice -n 10 "$backend/deploy/publish-release.sh" --product launcher --version "$version" \
    --platform "$platform" --channel "$channel" --notes-file "$notes" --zip "$zip" $replace
REMOTE
