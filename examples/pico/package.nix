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
    overrides = final: prev: {
      "platforms/raspberrypi" = prev."platforms/raspberrypi".overrideAttrs (_: {
      });
      "platforms/raspberrypi@src-d5b6f44125ad5e094769df758e95713f" =
        prev."platforms/raspberrypi@src-d5b6f44125ad5e094769df758e95713f".overrideAttrs
          (_: {
            sourceRoot = null;
            # nativeBuildInputs = [ libarchive ];
            # unpackPhase = ''
            #   bsdtar xf $src --strip-components=1
            # '';
            # postBuild = ''
            #   cp $out/platform.json "$out/package.json"
            # '';
          });
    };
  };
in
stdenv.mkDerivation {
  name = "pico";
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
