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
    let
      throwSystem = throw "${dep.name} unsupported system: ${stdenv.system}";
      src = dep.systems.${stdenv.system} or throwSystem;
    in
    stdenv.mkDerivation {
      pname = dep.name;
      version = dep.version;
      src = fetchurl src;
      sourceRoot = ".";

      env.MANIFEST = dep.manifest;
      buildPhase = ''
        mkdir -p $out
        mv * $out
        echo $MANIFEST > $out/.piopm
      '';

      passthru = {
        inherit (dep) manifest;
        installPath = dep.install_path;
        mutableInstall = false;
      };
    }
  ) (builtins.fromJSON (builtins.readFile lockfile)).dependencies;
  finalDeps = initialDeps // (overrides finalDeps initialDeps);
in
makeSetupHook
  {
    name = "platformio-setup-hook";
    passthru = {
      inherit finalDeps;
    };
  }
  (
    let
      # derived from `linkFarm`
      linkCommands = lib.mapAttrsToList (
        _: drv:
        let
          dest = "$PLATFORMIO_CORE_DIR/${drv.passthru.installPath}";
        in
        ''
          mkdir -p "$(dirname "${dest}")"
          ${
            if drv.passthru.mutableInstall then
              ''
                cp -Lr "${drv}" "${dest}"
                chmod -R +w "${dest}"
              ''
            else
              ''ln -s "${drv}" "${dest}"''
          }
        ''
      ) finalDeps;
    in
    writeShellScript "platformio-setup-hook.sh" ''
      _platformioSetupHook() {
        # top-level directory must be writable by PlatformIO
        export PLATFORMIO_CORE_DIR=./core-dir
        mkdir -p $PLATFORMIO_CORE_DIR
        ${lib.concatStrings linkCommands}
      }
      preConfigureHooks+=(_platformioSetupHook)
    ''
    // {
      passthru = {
        inherit finalDeps;
      };
    }
  )
