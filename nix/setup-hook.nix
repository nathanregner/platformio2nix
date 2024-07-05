{
  lib,
  makeSetupHook,
  stdenv,
  linkFarm,
  fetchurl,
  writeShellScript,
}:

{ lockfile }:

let
  deps = builtins.map (
    dep:
    let
      unpacked = stdenv.mkDerivation {
        pname = dep.name;
        version = dep.version;
        src = fetchurl dep.systems.${stdenv.system};
        sourceRoot = ".";

        env.MANIFEST = dep.manifest;
        buildPhase = ''
          mkdir -p $out
          mv * $out
          echo $MANIFEST > $out/.piopm
        '';
      };
    in
    {
      name = dep.name;
      path = unpacked;
    }
  ) (builtins.fromJSON (builtins.readFile lockfile)).dependencies;
  coreDir = linkFarm "platformio-core-dir" deps;
in
makeSetupHook
  {
    name = "platformio-setup-hook";
    passthru = {
      inherit coreDir;
    };
  }
  (
    writeShellScript "platformio-setup-hook.sh" ''
      _platformioSetupHook() {
        # top-level directory must be writable by PlatformIO
        export PLATFORMIO_CORE_DIR=$(mktemp -d)
        cp --no-deref -r ${coreDir}/* $PLATFORMIO_CORE_DIR
      }
      preConfigureHooks+=(_platformioSetupHook)
    ''
  )
