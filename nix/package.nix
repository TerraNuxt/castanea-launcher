# The launcher binary. Its window loads Vulkan, GL, Wayland, X11 and xkbcommon at run time (winit, wgpu), and the game it
# starts needs alsa. The wrapper puts them on LD_LIBRARY_PATH; the game inherits that environment. On NixOS the
# wrapper also appends the host's driver folder when it exists; the Vulkan and GL drivers come from the host.
{ lib
, rustPlatform
, pkg-config
, makeWrapper
, libxkbcommon
, wayland
, libGL
, vulkan-loader
, libX11
, libxcb
, libXcursor
, libXrandr
, libXi
, alsa-lib
, systemdLibs
}:

let
  runtimeLibs = [
    libxkbcommon
    wayland
    libGL
    vulkan-loader
    libX11
    libxcb
    libXcursor
    libXrandr
    libXi
    alsa-lib
    systemdLibs
  ];
in
rustPlatform.buildRustPackage {
  pname = "castanea-launcher";
  version = "0.1.0";

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "castanea-launcher" ];

  # The tests run in CI on Linux and Windows; the sandbox build skips them.
  doCheck = false;

  nativeBuildInputs = [ pkg-config makeWrapper ];
  buildInputs = runtimeLibs;

  postInstall = ''
    wrapProgram $out/bin/castanea-launcher \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs} \
      --run 'if [ -d /run/opengl-driver/lib ]; then export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}/run/opengl-driver/lib"; fi' \
      --run 'if [ -d /run/opengl-driver/share/vulkan/icd.d ]; then export XDG_DATA_DIRS="''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}:/run/opengl-driver/share"; fi'
    install -Dm644 packaging/linux/castanea-launcher.desktop -t $out/share/applications
    install -Dm644 packaging/linux/castanea-launcher.png \
      $out/share/icons/hicolor/512x512/apps/castanea-launcher.png
  '';

  passthru = { inherit runtimeLibs; };

  meta = {
    description = "Native launcher, mod browser and mod manager for castanea";
    homepage = "https://github.com/TerraNuxt/castanea-launcher";
    license = lib.licenses.mit;
    mainProgram = "castanea-launcher";
    platforms = lib.platforms.linux;
  };
}
