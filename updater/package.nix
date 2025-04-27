{
  rustPlatform,
  pkg-config,
  openssl,
}:

rustPlatform.buildRustPackage {
  name = "robotnix-updater";

  src = ./.;
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];

  cargoLock.lockFile = ./Cargo.lock;
}
