{
  fetchurl,
  lib,
  linkFarm,
  makeSetupHook,
  runCommand,
  stdenv,
  writeShellScript,
  writeTextFile,
}:

{
  lockfile,
  overrides ? (final: prev: { }),
}:

let
  inherit (builtins.fromJSON (builtins.readFile lockfile))
    dependencies
    integrityFiles
    ;
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
        inherit (dep) manifest installPath;
        mutableInstall = false;
      };
    }
  ) dependencies;
  finalDeps = initialDeps // (overrides finalDeps initialDeps);
  self =
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
          linkDeps = lib.mapAttrsToList (
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
          linkIntegrityFiles = lib.mapAttrsToList (
            installPath: contents:
            let
              file = writeTextFile {
                name = "integrity.dat";
                text = contents;
              };
            in
            ''
              mkdir -p "$PLATFORMIO_CORE_DIR/${installPath}"
              ln -s "${file}" "$PLATFORMIO_CORE_DIR/${installPath}/integrity.dat"
            ''
          ) integrityFiles;
        in
        writeShellScript "platformio-setup-hook.sh" ''
          _platformioSetupHook() {
            # top-level directory must be writable by PlatformIO
            export PLATFORMIO_CORE_DIR=./core-dir
            mkdir -p $PLATFORMIO_CORE_DIR
            set -x
            ${lib.concatStrings linkIntegrityFiles}
            ${lib.concatStrings linkDeps}
            set +x
          }
          preConfigureHooks+=(_platformioSetupHook)
        ''
      );
in
self
// {
  passthru = {
    inherit finalDeps;
    coreDir = stdenv.mkDerivation {
      pname = "debug-core-dir";
      version = "0.0.0";
      dontUnpack = true;

      nativeBuildInputs = [ self ];

      installPhase = ''
        mv $PLATFORMIO_CORE_DIR $out
      '';
    };
  };
}
