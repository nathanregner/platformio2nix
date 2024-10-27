{
  description = "Generate deterministic lockfiles for PlatformIO projects";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-parts,
      treefmt-nix,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      imports = [ inputs.treefmt-nix.flakeModule ];

      perSystem =
        {
          config,
          system,
          inputs',
          pkgs,
          lib,
          ...
        }:
        (
          let
            platformio2nix = pkgs.rustPlatform.buildRustPackage {
              pname = "platformio2nix";
              version = "0.1.1";
              src = ./cli;
              cargoLock.lockFile = ./cli/Cargo.lock;

              nativeBuildInputs =
                with pkgs;
                [ pkg-config ] ++ lib.optionals stdenv.isDarwin [ darwin.apple_sdk.frameworks.SystemConfiguration ];
              buildInputs = with pkgs; [ openssl ];
            };
          in
          {
            packages = rec {
              default = platformio2nix;
              inherit platformio2nix;
            };

            treefmt = import ./treefmt.nix;

            devShells.default = pkgs.mkShell {
              inherit (platformio2nix) nativeBuildInputs buildInputs;
              packages = (
                with pkgs;
                [
                  cargo
                  clippy
                  config.treefmt.build.wrapper
                  rust-analyzer
                  rustfmt
                ]
              );

              LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.openssl ];
              RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            };
          }
        );

      flake = {
        overlays.default = final: prev: {
          makePlatformIOSetupHook = final.callPackage ./setup-hook.nix { };
        };
      };
    };
}
