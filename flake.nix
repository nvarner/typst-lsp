{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    typst = {
      url = "github:typst/typst";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, fenix, nixpkgs, typst, flake-utils }:
    let
      inherit (builtins) substring;
      inherit (nixpkgs.lib) genAttrs importTOML optionals cleanSource;
    in
    flake-utils.lib.eachDefaultSystem (system:
      let
        fenixPkgs = fenix.packages.${system};
        pkgs = nixpkgs.legacyPackages.${system};

        toolchain = fenixPkgs.minimal.toolchain;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            (fenixPkgs.default.withComponents [
              "cargo"
              "clippy"
              "rustc"
              "rustfmt"
            ])
            fenixPkgs.rust-analyzer
            pkgs.nodejs
          ];

          buildInputs = optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.libiconv
          ];

          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;

          # Required by the jaeger feature
          nativeBuildInputs = [ pkgs.pkg-config ];
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        packages.default = rustPlatform.buildRustPackage {
          pname = "typst-lsp";
          inherit ((importTOML ./Cargo.toml).package) version;

          src = cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          preBuild = ''
            cp -r ${typst}/assets /build/cargo-vendor-dir/assets
          '';

          nativeBuildInputs = [
            pkgs.installShellFiles
          ];

          buildInputs = optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.CoreServices
          ];
        };
      });

}
