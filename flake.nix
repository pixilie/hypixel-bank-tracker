{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs.lib) genAttrs;

      forAllSystems = genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllPkgs = function: forAllSystems (system: function pkgs.${system});

      pkgs = forAllSystems (
        system:
        (import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        })
      );
    in
    {
      formatter = forAllPkgs (pkgs: pkgs.nixfmt-tree);

      packages = forAllPkgs (pkgs: rec {
        default = hypixel-bank-tracker;
        hypixel-bank-tracker = pkgs.callPackage ./package.nix { };
      });

      devShells = forAllPkgs (
        pkgs:
        with pkgs.lib;
        let
          file-rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rust-toolchain = file-rust-toolchain.override { extensions = [ "rust-src"  "rust-analyzer" ]; };
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.hypixel-bank-tracker ];
            packages = with pkgs; [
              rust-toolchain
              cargo-expand
            ];
          };
        }
      );

      nixosModules = rec {
        default = hypixel-bank-tracker;
        hypixel-bank-tracker = import ./module.nix self;
      };
    };
}
