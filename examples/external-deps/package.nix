{
  gnumake,
  libarchive,
  makePlatformIOSetupHook,
  platformio,
  stdenv,
  which,
}:
let
  version = "0.0.0";
  src = ./.;
  setupHook = makePlatformIOSetupHook {
    lockfile = ./platformio2nix.lock;
    overrides = (
      final: prev: {
        SlowSoftWire = prev.SlowSoftWire.overrideAttrs {
          nativeBuildInputs = [ libarchive ];
          unpackPhase = ''
            bsdtar xf $src --strip-components=1
          '';
        };
      }
    );
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
