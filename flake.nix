{
  description = "Terminal TUI markdown viewer with vim bindings, mermaid diagrams, and in-app document navigation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
      # rust-toolchain.toml stays the single source of the Rust version.
      toolchainFor = pkgs: pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          toolchain = toolchainFor pkgs;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          manifest = (lib.importTOML ./Cargo.toml).package;
        in
        {
          default = rustPlatform.buildRustPackage {
            pname = manifest.name;
            inherit (manifest) version;
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
                ./tests
                ./docs
                ./README.md
                (lib.fileset.fileFilter (f: lib.hasPrefix "sample" f.name) ./.)
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

            meta = {
              inherit (manifest) description homepage;
              license = lib.licenses.asl20;
              mainProgram = "bmd";
            };
          };
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
          packages = [
            ((toolchainFor pkgs).override {
              extensions = [ "rust-src" ];
              targets = [
                "x86_64-unknown-linux-musl"
                "aarch64-unknown-linux-musl"
              ];
            })
            pkgs.prek
            pkgs.sccache
            pkgs.zig
            pkgs.cargo-zigbuild
          ];
          shellHook = ''
            # Keep cargo state (registry, crates.io token) and sccache inside the checkout.
            export CARGO_HOME="$PWD/.cargo"
            export SCCACHE_DIR="$PWD/.sccache"
            export RUSTC_WRAPPER=sccache
            mkdir -p "$CARGO_HOME" "$SCCACHE_DIR"
          '';
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
