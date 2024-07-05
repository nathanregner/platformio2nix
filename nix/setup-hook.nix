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
        buildPhase = ''
          ls -l
          mkdir -p $out
          mv * $out
        '';
      };
    in
    {
      name = "${dep.type}s/${dep.name}";
      path = unpacked;
    }
  ) lockfile.dependencies;
  coreDir = linkFarm "platformio-core-dir" deps;
in
makeSetupHook { name = "platformio-setup-hook"; } (
  writeShellScript "platformio-setup-hook.sh" ''
    _platformioSetupHook() {
      # TODO: remove
      echo 'testing testing testing'
      export PLATFORMIO_CORE_DIR=$(mktemp -d)
      cp -r --no-dereference ${coreDir}/* $PLATFORMIO_CORE_DIR
    }
    preConfigureHooks+=(_platformioSetupHook)
  ''
)
