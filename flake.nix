{
  inputs = {
    fenix.url = "github:nix-community/fenix";
    nixpkgs.follows = "fenix/nixpkgs";
    # it makes more sense for us to follow the nixpkgs of fenix than making
    # fenix follow nixpkgs because nix-community's cachix probably has our toolchain
    # cached, we don't want to unnecessarily build it.
    typst = {
      url = "github:typst/typst/v0.7.0";
      flake = false;
    };
    typst-fmt = {
      url = "github:astrale-sharp/typstfmt/8404a15";
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = inputs @ {
    fenix,
    nixpkgs,
    flake-utils,
    ...
  }: let
    inherit (nixpkgs.lib) importTOML optionals cleanSource;
  in
    flake-utils.lib.eachDefaultSystem (system: let
      fenixPkgs = fenix.packages.${system};
      pkgs = nixpkgs.legacyPackages.${system};

      toolchain = fenixPkgs.minimal.toolchain;
      rustPlatform = pkgs.makeRustPlatform {
        cargo = toolchain;
        rustc = toolchain;
      };
      PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
    in {
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

        inherit PKG_CONFIG_PATH;
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;

        # Required by the jaeger feature
        nativeBuildInputs = [pkgs.pkg-config];
      };

      packages.default = rustPlatform.buildRustPackage {
        pname = "typst-lsp";

        inherit PKG_CONFIG_PATH;
        inherit ((importTOML ./Cargo.toml).package) version;

        doCheck = false;

        src = cleanSource ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
          allowBuiltinFetchGit = true;
        };

        preBuild = ''
          cp -r ${inputs.typst}/assets /build/cargo-vendor-dir/assets
          cp -r ${inputs.typst-fmt}/README.md /build/cargo-vendor-dir
        '';

        nativeBuildInputs = [
          pkgs.installShellFiles
          pkgs.pkg-config
        ];

        buildInputs = optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];
      };
    });
}
