#!/bin/bash
# Builds the launcher executable for one platform and checks it: tools/build_binary.sh <linux-x86_64|windows-x86_64> <outdir>
# The same script runs in the build host's task container (tools/release.sh) and on a GitHub Actions runner
# (.github/workflows/release-build.yml), so both make the same bytes from the same tree and toolchain.
#   linux-x86_64     cargo build --release for the host target; the result must answer `--version`
#   windows-x86_64   cargo build --release for x86_64-pc-windows-gnu (mingw-w64 with posix threads, windres); the result
#                    must be a PE32+ GUI program with the icon and version resources and no DLL outside Windows itself
# Reproducible: --locked, source paths mapped to fixed names, no link timestamp. It needs the mingw toolchain
# (tools/builder/Dockerfile lists it) only for windows-x86_64.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
die() { echo "build_binary.sh: $*" >&2; exit 2; }
[ $# -eq 2 ] || { sed -n '2,/^set /p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2; exit 2; }
platform=$1
out=$2
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$root"

version=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' Cargo.toml)
target_dir=${CARGO_TARGET_DIR:-$root/target}
cargo_home=${CARGO_HOME:-$HOME/.cargo}
flags="--remap-path-prefix=$root=/src --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$target_dir=/target"

case $platform in
    linux-x86_64)
        RUSTFLAGS="$flags" cargo build --release --locked -p castanea-launcher
        bin=$target_dir/release/castanea-launcher
        said=$("$bin" --version)
        [ "$said" = "castanea-launcher $version" ] || die "the binary says '$said', expected 'castanea-launcher $version'"
        cp "$bin" "$out/castanea-launcher"
        ;;
    windows-x86_64)
        target=x86_64-pc-windows-gnu
        RUSTFLAGS="$flags -C link-arg=-Wl,--no-insert-timestamp" cargo build --release --locked -p castanea-launcher --target "$target"
        bin=$target_dir/$target/release/castanea-launcher.exe
        info=$(x86_64-w64-mingw32-objdump -p "$bin")
        grep -q 'Subsystem.*(Windows GUI)' <<<"$info" || die "the exe is not a Windows GUI program"
        grep -q 'Resource Directory \[.rsrc\]' <<<"$info" || grep -q '\.rsrc' <<<"$info" || die "the exe has no resources (icon, version)"
        strings -el "$bin" | grep -q 'castanea launcher' || die "the version info is not in the exe"
        dlls=$(sed -n 's/^\s*DLL Name: //p' <<<"$info" | tr 'A-Z' 'a-z' | sort -u)
        # The mingw runtime is linked in statically; a DLL of it next to the exe would be missing on a clean Windows.
        extra=$(grep -Ex 'libwinpthread-1\.dll|libgcc_s_[a-z]+-1\.dll|libstdc\+\+-6\.dll|libssp-0\.dll' <<<"$dlls" || true)
        [ -z "$extra" ] || die "the exe needs mingw runtime DLLs that Windows does not have: $extra"
        cp "$bin" "$out/castanea-launcher.exe"
        ;;
    *) die "platform must be linux-x86_64 or windows-x86_64" ;;
esac
echo "build_binary.sh: $platform $version $(sha256sum "$out"/castanea-launcher* | cut -c1-16)" >&2
