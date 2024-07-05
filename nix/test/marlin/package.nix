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
in
stdenv.mkDerivation {
  name = "marlin";
  inherit version src;

  nativeBuildInputs = [
    (makePlatformIOSetupHook {
      lockfile = builtins.fromJSON (builtins.readFile ./platformio2nix.lock);
    })
  ];

  buildInputs = [
    gnumake
    platformio
    which
  ];

  patchPhase = ''
    patchShebangs ./buildroot/bin
  '';

  buildPhase = ''
    echo $PLATFORMIO_CORE_DIR
    yes 1 | make marlin
  '';
}