{
  description = "A `kubectl port-forward` replacement";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    # rust
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay.url = "github:oxalica/rust-overlay";

    # checks/formatting
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      advisory-db,
      crane,
      flake-utils,
      rust-overlay,
      treefmt-nix,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay)
            # lazy hack to work around https://github.com/rust-lang/rust-bindgen/issues/2488
            (final: prev: { clang = prev.clang_15; })
          ];
        };

        inherit (pkgs) lib;

        rustToolchainWith =
          extensions:
          pkgs.rust-bin.selectLatestNightlyWith (
            toolchain: toolchain.default.override { inherit extensions; }
          );
        rustToolchain = rustToolchainWith [ ];
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter =
            path: type:
            builtins.match ".*resolve1.xml$" path != null || (craneLib.filterCargoSources path type);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;

          nativeBuildInputs = lib.optionals pkgs.stdenv.isLinux (
            with pkgs;
            [
              #
              rustPlatform.bindgenHook
              pkg-config
              openssl
            ]
          );

          buildInputs =
            [ ]
            ++ lib.optionals pkgs.stdenv.isLinux (with pkgs; [ dbus ])
            ++ lib.optionals pkgs.stdenv.isDarwin (
              with pkgs;
              [
                libiconv
                darwin.apple_sdk.frameworks.CoreServices
                darwin.apple_sdk.frameworks.Security
              ]
            );
        };

        # Build *just* the cargo dependencies
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency artifacts from
        # above
        konnect = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
          }
        );

        treefmt = treefmt-nix.lib.evalModule pkgs (
          import ./treefmt.nix { rustfmt = rustToolchain.passthru.availableComponents.rustfmt; }
        );
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          crate = konnect;

          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          crate-doc = craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });

          # Check formatting
          formatting = treefmt.config.build.check self;

          # Audit dependencies
          audit = craneLib.cargoAudit { inherit src advisory-db; };

          # Audit licenses
          deny = craneLib.cargoDeny { inherit src; };

          # Run tests with cargo-nextest. Set `doCheck = false` to prevent tests running twice
          nextest = craneLib.cargoNextest (commonArgs // { inherit cargoArtifacts; });
        };

        packages = rec {
          default = konnect;
          inherit konnect;

          ociImage = pkgs.ociTools.buildContainer { args = [ "${konnect}/bin/konnect" ]; };
          dockerImage = pkgs.dockerTools.buildImage {
            name = "hello";
            tag = "latest";
            created = "now";
            copyToRoot = pkgs.buildEnv {
              name = "image-root";
              paths = [ pkgs.hello ];
              pathsToLink = [ "/bin" ];
            };
            config.Cmd = [ "/bin/hello" ];
          };

          vm =
            (nixpkgs.lib.nixosSystem {
              modules = [
                (
                  { modulesPath, ... }:
                  let
                    toGuest = builtins.replaceStrings [ "darwin" ] [ "linux" ];
                    pkgs = nixpkgs.legacyPackages.${system};
                  in
                  {
                    imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

                    virtualisation.host = {
                      inherit pkgs;
                    };

                    nixpkgs.hostPlatform = toGuest pkgs.stdenv.hostPlatform.system;
                  }
                )
                ./vm.nix
              ];
              # inherit system;
            }).config.system.build.vm;
        };

        apps.default = flake-utils.lib.mkApp { drv = konnect; };

        formatter = treefmt.config.build.wrapper;

        devShells.default =
          let
            rustToolchain = (
              rustToolchainWith [
                "rust-src"
                "rust-analyzer"
              ]
            );
          in
          (craneLib.overrideToolchain rustToolchain).devShell {
            checks = self.checks.${system}; # inherit inputs from checks
            packages = [ treefmt.config.build.programs.nixfmt-rfc-style ];
            RUSTFLAGS = "--cfg tokio_unstable";
            RUST_SRC_PATH = "${rustToolchain.passthru.availableComponents.rust-src}";
          };
      }
    );
}
