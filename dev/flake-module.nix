{
  inputs,
  config,
  lib,
  ...
}:
{
  imports = [
    inputs.flake-root.flakeModule
    inputs.treefmt-nix.flakeModule
  ];

  systems = [
    "aarch64-darwin"
    "aarch64-linux"
    "x86_64-darwin"
    "x86_64-linux"
  ];

  perSystem =
    {
      config,
      pkgs,
      system,
      ...
    }:
    {
      treefmt = import ./treefmt.nix;

      checks = lib.mergeAttrsList (
        lib.mapAttrsToList (
          example: outputs:
          lib.mapAttrs' (name: check: {
            name = "${example}/${name}";
            value = check;
          }) outputs.checks.${system}
        ) inputs.examples
      );

      devShells.default = pkgs.mkShell {
        inputsFrom = [
          config.flake-root.devShell
          config.treefmt.build.devShell
          inputs.platformio2nix.packages.${system}.platformio2nix
        ];
        packages = with pkgs; [
          cargo
          clippy
          rust-analyzer
          rustfmt
          platformio

          # for convenience when updating examples
          (writeShellScriptBin "platformio2nix" ''
            cargo run --manifest-path "$FLAKE_ROOT/cli/Cargo.toml" -- "$@"
          '')

          (pkgs.writeShellScriptBin "check-examples" ''
            for dir in "$FLAKE_ROOT"/examples/*; do
              nix flake check "$dir"
            done
          '')
        ];

        LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.openssl.dev ];
        PLATFORMIO_CORE_DIR = ".pio";
        RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
      };
    };

  flake = {
    config.config = config;
  };
}
