{
  lib,
  fetchurl,
  makeSetupHook,
  stdenv,
  writeShellScript,
  writeShellScriptBin,
}:

{
  lockfile,
  overrides ? (final: prev: { }),
}:

let
  inherit (builtins.fromJSON (builtins.readFile lockfile)) version dependencies;
  initialDeps = builtins.mapAttrs (
    installPath: dep:
    let
      throwSystem = throw "${dep.name} unsupported system: ${stdenv.system}: ${builtins.attrNames dep.src}";
      src = dep.src.universal or dep.src.systems.${stdenv.system} or throwSystem;
    in
    stdenv.mkDerivation {
      pname = dep.name;
      version = dep.manifest.version;
      src = fetchurl src;
      sourceRoot = ".";

      env.MANIFEST = builtins.toJSON dep.manifest;
      buildPhase = ''
        runHook preBuild
        mkdir -p "$out"
        mv * "$out"
        echo "$MANIFEST" >"$out/.piopm"
        runHook postBuild
      '';

      passthru = {
        inherit (dep) manifest;
        inherit installPath;
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
          run = writeShellScriptBin "run" ''
            source ${self}/nix-support/setup-hook
            _platformioSetupHook
          '';
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
            export PLATFORMIO_CORE_DIR=./.pio
            export PLATFORMIO_WORKSPACE_DIR=./.pio
            # top-level directory must be writable by PlatformIO
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
      );
in

assert lib.assertMsg (version == "2") ''Unsupported lockfile version "${version}"'';

self
