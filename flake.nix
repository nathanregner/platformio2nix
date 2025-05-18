{
  description = "Generate deterministic lockfiles for PlatformIO projects";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    {
      self,
      flake-parts,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      imports = [ inputs.flake-parts.flakeModules.partitions ];

      partitionedAttrs = {
        checks = "dev";
        devShells = "dev";
        formatter = "dev";
      };
      partitions.dev = {
        extraInputsFlake = ./dev;
        extraInputs = {
          platformio2nix = self;
          examples =
            let
              importExample = path: (import path).outputs { platformio2nix = self; };
            in
            {
              external-deps = importExample ./examples/external-deps/flake.nix;
              marlin = importExample ./examples/marlin/flake.nix;
              multi-env = importExample ./examples/multi-env/flake.nix;
            };
        };
        module = {
          imports = [ ./dev/flake-module.nix ];
        };
      };

      perSystem =
        { pkgs, ... }:
        {
          legacyPackages = {
            makePlatformIOSetupHook = pkgs.callPackage ./setup-hook.nix { };
          };

          packages = rec {
            platformio2nix = pkgs.callPackage ./package.nix { };
            default = platformio2nix;
          };
        };

      flake = {
        overlays.default = final: prev: {
          inherit (self.legacyPackages.${final.system})
            makePlatformIOSetupHook
            platformio2nix
            ;
        };
      };
    };
}
