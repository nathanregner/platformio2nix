{
  lib,
  makeSetupHook,
  stdenv,
  linkFarm,
  fetchurl,
  writeShellScript,
}:

{
  lockfile,
  overrides ? (final: prev: { }),
}:

let
  initialDeps = builtins.mapAttrs (
    _: dep:
    stdenv.mkDerivation {
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

      passthru = {
        inherit (dep) install_path manifest;
      };
    }
  ) (builtins.fromJSON (builtins.readFile lockfile)).dependencies;
  finalDeps = initialDeps // (overrides finalDeps initialDeps);
  coreDir = linkFarm "platformio-core-dir" (
    lib.mapAttrsToList (_: drv: {
      name = drv.passthru.install_path;
      path = drv;
    }) finalDeps
  );
in
makeSetupHook
  {
    name = "platformio-setup-hook";
    passthru = {
      inherit coreDir finalDeps;
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
