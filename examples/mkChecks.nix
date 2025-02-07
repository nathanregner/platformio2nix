{
  packages,
  platformio2nix,
  src,
}:
builtins.mapAttrs (
  system:
  { default }:
  (
    let
      pkgs = import platformio2nix.inputs.nixpkgs {
        inherit system;
        overlays = [ platformio2nix.overlays.default ];
      };
    in
    {
      package = default;

      deterministicLockfile =
        pkgs.runCommand "deterministicLockfile"
          {
            nativeBuildInputs = [
              pkgs.platformio2nix
              default.passthru.setupHook
            ];
          }
          ''
            set -e
            _platformioSetupHook
            platformio2nix --cache-dir ${src}/../.cache > $out
            diff ${src}/platformio2nix.lock $out
          '';
    }
  )
) packages
