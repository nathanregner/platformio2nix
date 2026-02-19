{
  lib,
  fetchgit,
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

  fetchers = {
    git = fetchgit;
    url = fetchurl;
  };

  initialDeps = builtins.mapAttrs (
    installPath: dep:
    let
      throwSystem = throw "${dep.name} unsupported system: ${stdenv.hostPlatform.system}: ${builtins.attrNames dep.src}";
      universal = dep.src.universal or null;
      fetcher = fetchers.${universal.type or "url"};
      src = fetcher (
        if universal != null then
          removeAttrs universal [ "type" ]
        else
          dep.src.systems.${stdenv.hostPlatform.system} or throwSystem
      );
    in
    stdenv.mkDerivation {
      pname = dep.name;
      inherit (dep.manifest) version;
      inherit src;

      # fix "unpacker produced multiple directories" for registry packages
      # generally this doesn't seem to be required for external packages, but it can be overridden
      sourceRoot = if dep.manifest.spec.uri == null then "." else null;

      # skip patching toolchain-gccarmnoneeabi, etc
      # again, if this is wrong, it can be overridden
      dontFixup = true;

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
                  ''
                    ln -s "${drv}" "${dest}"
                  ''
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
