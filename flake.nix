{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          inherit (pkgs) lib;
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          src = craneLib.cleanCargoSource ./.;

          nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ];
          buildInputs = with pkgs; [ openssl ];
          commonArgs = {
            inherit src buildInputs nativeBuildInputs;
            pname = "liquid-asset-registry";
            doCheck = false;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          bin = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--features cli,server,client,dev --bins";
          });

          dockerImage = pkgs.dockerTools.streamLayeredImage {
            name = "xenoky/liquid-asset-registry";
            tag = "02843eb2";
            contents = [ bin ];
            config = {
              Cmd = [ "${bin}/bin/server" ];
            };
          };

        in
        with pkgs;
        {
          packages =
            {
              inherit bin dockerImage;
              default = bin;
            };
          apps."server" = {
            type = "app";
            program = "${bin}/bin/server";
          };

          devShells.default = mkShell {
            inputsFrom = [ bin ];

            buildInputs = with pkgs; [ ];
          };
        }
      );
}
