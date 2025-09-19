{
  openssl,
  pkg-config,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "platformio2nix";
  version = "0.2.0";
  src = ./cli;
  cargoLock.lockFile = ./cli/Cargo.lock;

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];
}
