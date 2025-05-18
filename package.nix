{
  lib,
  darwin,
  openssl,
  pkg-config,
  rustPlatform,
  stdenv,
}:
rustPlatform.buildRustPackage {
  pname = "platformio2nix";
  version = "0.2.0";
  src = ./cli;
  cargoLock.lockFile = ./cli/Cargo.lock;

  nativeBuildInputs = [
    pkg-config
  ] ++ lib.optionals stdenv.isDarwin [ darwin.apple_sdk.frameworks.SystemConfiguration ];
  buildInputs = [ openssl ];
}
