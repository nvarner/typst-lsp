{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    inherit
      (builtins)
      substring
      ;
    inherit
      (nixpkgs.lib)
      genAttrs
      optionals
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
      pkgs.rustPlatform.buildRustPackage {
        pname = "typst-lsp";
        version = rev "00000000";

        src = self;

        cargoLock = {
          lockFile = ./Cargo.lock;
          allowBuiltinFetchGit = true;
        };

        buildInputs = optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];
      };
  in {
    devShells = eachSystem (pkgs: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          clippy
          rust-analyzer
          rustc
          rustfmt
          nodejs
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
