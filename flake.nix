{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    fenix,
    nixpkgs,
  }: let
    inherit
      (builtins)
      substring
      ;
    inherit
      (nixpkgs.lib)
      genAttrs
      importTOML
      optionals
      cleanSource
      ;

    eachSystem = f:
      genAttrs
      [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ]
      (system: f nixpkgs.legacyPackages.${system});

    rev = fallback:
      if self ? rev
      then substring 0 8 self.rev
      else fallback;

    packageFor = pkgs:
      let
        rust = fenix.packages.${pkgs.stdenv.hostPlatform.system}.minimal.toolchain;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
      in
      rustPlatform.buildRustPackage {
        pname = "typst-lsp";
        inherit ((importTOML ./Cargo.toml).workspace.package) version;

        src = cleanSource ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
          allowBuiltinFetchGit = true;
        };

        nativeBuildInputs = [
          pkgs.installShellFiles
        ];

        buildInputs = optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];
      };
  in {
    devShells = eachSystem (pkgs: {
      default = pkgs.mkShell {
        packages =
          let
            fenix' = fenix.packages.${pkgs.stdenv.hostPlatform.system};
          in [
            (fenix'.default.withComponents [
              "cargo"
              "clippy"
              "rustc"
              "rustfmt"
            ])
            fenix'.rust-analyzer
            pkgs.nodejs
          ];

        buildInputs = optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.CoreServices
          pkgs.libiconv
        ];

        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });

    packages = eachSystem (pkgs: {
      default = packageFor pkgs;
    });
  };
}
