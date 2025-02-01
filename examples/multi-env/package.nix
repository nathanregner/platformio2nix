{
  gnumake,
  makePlatformIOSetupHook,
  platformio,
  stdenv,
  which,
  ...
}:
let
  version = "0.0.0";
  src = ./.;
  setupHook = makePlatformIOSetupHook {
    lockfile = ./platformio2nix.lock;
  };
in
stdenv.mkDerivation {
  name = "uno";
  inherit version src;

  nativeBuildInputs = [ setupHook ];

  buildInputs = [
    gnumake
    platformio
    which
  ];

  buildPhase = ''
    platformio run
  '';

  installPhase = ''
    mkdir -p $out
    cp -r .pio/build/* $out
  '';

  passthru = {
    inherit setupHook;
  };
}
