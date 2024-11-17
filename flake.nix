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
      self,
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
              version = "0.2.0";
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
              packages =
                [ config.treefmt.build.wrapper ]
                ++ (with pkgs; [
                  cargo
                  clippy
                  rust-analyzer
                  rustfmt
                  platformio

                  # for convenience when updating examples
                  (writeShellScriptBin "platformio2nix" ''
                    cargo run --manifest-path "$FLAKE_ROOT/cli/Cargo.toml" -- "$@"
                  '')
                ]);

              LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.openssl ];
              PLATFORMIO_CORE_DIR = ".pio";
              RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            };
          }
        );

      flake = {
        overlays.default = final: prev: {
          inherit (self.packages.${final.system}) platformio2nix;
          makePlatformIOSetupHook = final.callPackage ./setup-hook.nix { };
        };
      };
    };
}
