#!/bin/sh
# Windows cross-check from Linux: syncs the tree to xenoo and runs cargo check and clippy for a Windows target in a
# throwaway container (mingw for ring's C; nothing is linked or run). Like castanea's `tools/cargo-remote winck`.
#   tools/winck.sh [x86_64-pc-windows-gnu|x86_64-pc-windows-msvc]
set -eu
TARGET=${1:-x86_64-pc-windows-gnu}
HOST=${LAUNCHER_REMOTE:-xenoo}
IMAGE=${WINCK_IMAGE:-castanea-builder:nextest}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
rsync -a --delete --exclude /target --exclude /.git --exclude /.winck "$ROOT/" "$HOST:castanea-launcher-build/"
SCRIPT='set -e
T=$1
rustup target add "$T" >/dev/null
apt-get update -qq >/dev/null
apt-get install -y -qq gcc-mingw-w64-x86-64 llvm >/dev/null
case $T in *-msvc) export CC_x86_64_pc_windows_msvc=x86_64-w64-mingw32-gcc AR_x86_64_pc_windows_msvc=llvm-lib ;; esac
cargo check --workspace --all-targets --target "$T"
cargo clippy --workspace --all-targets --target "$T" -- -D warnings
echo "winck: PASS ($T)"'
exec ssh -o BatchMode=yes "$HOST" "B=\$HOME/castanea-launcher-build; mkdir -p \$B/.winck/target \$B/.winck/cargo-home; \
    nice -n 10 docker run --rm --init --cpus 6 --memory 8g -w /work -v \$B:/work \
    -v \$B/.winck/target:/target -v \$B/.winck/cargo-home:/cargo-home -e CARGO_TARGET_DIR=/target -e CARGO_HOME=/cargo-home \
    $IMAGE bash -c '$SCRIPT' winck $TARGET"
