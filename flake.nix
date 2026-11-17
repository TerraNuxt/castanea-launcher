{
  description = "castanea launcher: native game launcher and mod manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs {
        inherit system;
        overlays = [ self.overlays.default ];
      }));
    in
    {
      overlays.default = final: _prev: {
        castanea-launcher = final.callPackage ./nix/package.nix { };
      };

      packages = forAll (pkgs: {
        default = pkgs.castanea-launcher;
        castanea-launcher = pkgs.castanea-launcher;
      });

      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = "${pkgs.castanea-launcher}/bin/castanea-launcher";
          meta.description = "Start the castanea launcher";
        };
      });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ pkgs.castanea-launcher ];
          packages = with pkgs; [ cargo rustc clippy rustfmt rust-analyzer ];
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath pkgs.castanea-launcher.runtimeLibs;
        };
      });
    };
}
