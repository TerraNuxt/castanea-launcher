# The launcher binary. Its window loads Vulkan, GL, Wayland, X11 and xkbcommon at run time (winit, wgpu),
# so the wrapper puts them on LD_LIBRARY_PATH; the Vulkan and GL drivers come from the host.
{ lib
, rustPlatform
, pkg-config
, makeWrapper
, libxkbcommon
, wayland
, libGL
, vulkan-loader
, libX11
, libXcursor
, libXrandr
, libXi
}:

let
  runtimeLibs = [
    libxkbcommon
    wayland
    libGL
    vulkan-loader
    libX11
    libXcursor
    libXrandr
    libXi
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
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs}
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
