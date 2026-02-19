{
  lib,
  makeWrapper,
  nix-prefetch-git,
  openssl,
  pkg-config,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "platformio2nix";
  version = "0.2.0";
  src = ./cli;
  cargoLock.lockFile = ./cli/Cargo.lock;

  nativeBuildInputs = [
    makeWrapper
    pkg-config
  ];
  buildInputs = [ openssl ];

  postInstall = ''
    wrapProgram $out/bin/platformio2nix \
      --prefix PATH : ${lib.makeBinPath [ nix-prefetch-git ]}
  '';
}
