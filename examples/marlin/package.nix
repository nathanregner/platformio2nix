{
  fetchFromGitHub,
  gnumake,
  makePlatformIOSetupHook,
  platformio,
  stdenv,
  which,
}:
let
  version = "2.1.2.4";
  src = fetchFromGitHub {
    owner = "MarlinFirmware";
    repo = "Marlin";
    rev = version;
    fetchSubmodules = false;
    sha256 = "sha256-OQ7bUvc2W54UqzsoxgATQg3yl1v9e+8duJI7bL2fvII=";
  };
  setupHook = makePlatformIOSetupHook {
    lockfile = ./platformio2nix.lock;
    overrides = (
      final: prev: {
        "packages/toolchain-atmelavr" = prev."packages/toolchain-atmelavr".overrideAttrs (drv: {
          dontFixup = stdenv.hostPlatform.isDarwin;
        });
      }
    );
  };
in
stdenv.mkDerivation {
  name = "marlin";
  inherit version src;

  nativeBuildInputs = [
    gnumake
    platformio
    setupHook
    which
  ];

  patchPhase = ''
    patchShebangs ./buildroot/bin
    substituteInPlace buildroot/bin/mftest \
      --replace-fail 'pio run $SILENT_FLAG -e $TARGET' 'pio run -v -e $TARGET'
  '';

  buildPhase = ''
    echo '1' | make marlin
  '';

  installPhase = ''
    mkdir -p $out
    cp -r .pio/build/* $out
  '';

  passthru = {
    inherit setupHook;
  };
}
