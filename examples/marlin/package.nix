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
  setupHook = makePlatformIOSetupHook { lockfile = ./platformio2nix.lock; };
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
  '';

  buildPhase = ''
    echo "PLATFORMIO_CORE_DIR: $PLATFORMIO_CORE_DIR"
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
