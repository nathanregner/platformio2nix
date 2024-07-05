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
          ${if drv.passthru.mutableInstall then ''cp -Lr ${drv} ${dest}'' else ''ln -s ${drv} ${dest}''}
        ''
      ) finalDeps;
    in
    writeShellScript "platformio-setup-hook.sh" ''
      _platformioSetupHook() {
        # top-level directory must be writable by PlatformIO
        export PLATFORMIO_CORE_DIR=$(mktemp -d)
        ${lib.concatStrings linkCommands}
      }
      preConfigureHooks+=(_platformioSetupHook)
    ''
  )
