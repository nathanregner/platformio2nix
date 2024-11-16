{
  description = "SimpleFOC build example";

  inputs = {
    platformio2nix.url = "../..";
    # nixpkgs.follows = "platformio2nix.inputs.nixpkgs";
  };

  outputs =
    { platformio2nix, ... }:
    let
      nixpkgs = platformio2nix.inputs.nixpkgs;
      inherit (nixpkgs) lib;
      forAllSystems = lib.genAttrs [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ platformio2nix.overlays.default ];
          };
        in
        {
          default = pkgs.callPackage ./package.nix { };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ platformio2nix.overlays.default ];
          };
        in
        {
          default = pkgs.mkShell {
            buildInputs = [
              pkgs.platformio
              pkgs.platformio2nix
            ];
            shellHook = ''
              export PLATFORMIO_CORE_DIR="$(pwd)/.pio";
            '';
          };
        }
      );
    };
}
