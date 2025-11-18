{ lib

, rustPlatform
, gitignore

, makeWrapper
, openssl
, pkg-config
}:

let
  inherit (gitignore.lib) gitignoreSource;

  src = gitignoreSource ./.;
  cargoTOML = lib.importTOML "${src}/Cargo.toml";
in
rustPlatform.buildRustPackage {
  pname = cargoTOML.package.name;
  version = cargoTOML.package.version;

  inherit src;

  cargoLock = { lockFile = "${src}/Cargo.lock"; };

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    openssl
  ];

  postInstall = ''
    # Copy static assets
    install -Dm755 -d "$out/share/hypixel-bank-tracker/static"
    cp -r "${src}/static/"* "$out/share/hypixel-bank-tracker/static/"

    # Wrap the program so it sees the static files
    wrapProgram "$out/bin/hypixel-bank-tracker" \
      --set HBT_STATIC_FOLDER "$out/share/hypixel-bank-tracker/static"
  '';

  meta = {
    inherit (cargoTOML.package)
      description
      homepage
      # license
      ;
    maintainers = cargoTOML.package.authors;
    mainProgram = "hypixel-bank-tracker";
  };
}
