# The launcher's build image on the build host: the game's builder image plus the Windows cross toolchain (mingw-w64
# with posix threads, windres for the icon and version resource), cargo-deny and zip. Built once per content of this
# file by tools/lib/xenoo.sh (tag castanea-launcher-builder:<hash of this file>), so a release never installs packages.
FROM castanea-builder:nextest
RUN apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends gcc-mingw-w64-x86-64-posix binutils-mingw-w64-x86-64 zip \
 && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-gnu
RUN cargo install cargo-deny --locked --root /usr/local \
 && rm -rf /cargo-home/registry /cargo-home/git /tmp/*
