#!/bin/bash
# Windows cross-check from Linux: syncs the tree to the build host and runs cargo check and clippy for a Windows target in
# the task container (mingw for ring's C; nothing is linked or run). Like castanea's `tools/cargo-remote winck`.
#   tools/winck.sh [x86_64-pc-windows-gnu|x86_64-pc-windows-msvc]
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/lib/xenoo.sh"
xenoo_init "$root"
target=${1:-x86_64-pc-windows-gnu}
script='set -e
T=$1
rustup target add "$T" >/dev/null
case $T in *-msvc) export CC_x86_64_pc_windows_msvc=x86_64-w64-mingw32-gcc AR_x86_64_pc_windows_msvc=llvm-lib ;; esac
cargo check --workspace --all-targets --target "$T"
cargo clippy --workspace --all-targets --target "$T" -- -D warnings
echo "winck: PASS ($T)"'
xenoo_sync
xenoo_image
xenoo_run "$script" "$target"
