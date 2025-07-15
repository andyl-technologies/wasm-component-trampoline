{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";

    flake-parts.url = "github:hercules-ci/flake-parts";

    crane.url = "github:ipetkov/crane";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { flake-parts, nixpkgs, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        {
          self',
          pkgs,
          lib,
          system,
          ...
        }:
        let
          rustToolchain =
            with inputs.fenix.packages.${system};
            fromToolchainFile {
              dir = ./.;
              sha256 = "9h1rTC6CFU7/Q1ltpCZ9rKVnY8WzwAktn0/6PANIWgs=";
            };
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

          witFilter = path: _type: builtins.match ".*wit$" path != null;
          srcFilter = path: type: (witFilter path type) || (craneLib.filterCargoSources path type);

          src = lib.cleanSourceWith {
            src = ./.;
            filter = srcFilter;
            name = "source";
          };
          versionInfo = craneLib.crateNameFromCargoToml { inherit src; };
          commonArgs = {
            inherit (versionInfo) pname version;
            inherit src;

            nativeBuildInputs = [
              pkgs.rustPlatform.bindgenHook
            ];

            strictDeps = true;
            doCheck = false;
          };

          cargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              cargoExtraArgs = "--workspace";
            }
          );

          commonCheckArgs = commonArgs // {
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.cargo-nextest ];
            inherit cargoArtifacts;
            doCheck = true;
          };
        in
        {
          _module.args = {
            inherit craneLib;

            pkgs = import nixpkgs {
              inherit system;
              overlays = [
                (import ./nix/cargo-llvm.nix rustToolchain)
              ];
            };
          };

          devShells.default = craneLib.devShell {
            name = "wasm-component-trampoline-shell";

            inherit rustToolchain;

            packages = with pkgs; [
              rustToolchain
              cargo-nextest
              cargo-vet
              cargo-watch

              wasm-tools
              wasmtime
            ];
          };

          packages = {
            runner = craneLib.mkCargoDerivation (
              commonArgs
              // {
                pname = "runner";
                inherit cargoArtifacts;
                nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.wasm-tools ];

                buildPhaseCargoCommand = ''
                  pushd tests/runner
                  cargo build --release
                  popd

                  for x in kvstore logger application; do
                    cargo build --target wasm32-unknown-unknown --release -p "$x"
                    wasm-tools component new \
                      "./target/wasm32-unknown-unknown/release/$x.wasm" \
                      > "$x.component.wasm"
                  done
                '';

                installPhase = ''
                  runHook preInstall

                  mkdir -p $out/bin $out/share

                  cp target/release/{async-runner,runner} $out/bin
                  cp *.component.wasm $out/share

                  runHook postInstall
                '';
              }
            );
          };

          checks = {
            cargo-fmt = craneLib.cargoFmt (
              commonCheckArgs
              // {
                cargoExtraArgs = "--all";
              }
            );

            # To run this test properly, use `nix run .#checks.<arch>.test-runners`
            test-runners = pkgs.writeShellApplication {
              name = "run-wasm-entrypoint";
              runtimeInputs = [
                self'.packages.runner
              ];
              text = ''
                WASM_ARTIFACTS="${self'.packages.runner}/share"

                runner -w "$WASM_ARTIFACTS"
                async-runner -w "$WASM_ARTIFACTS"
              '';
            };

            cargo-nextest = craneLib.cargoNextest (
              commonCheckArgs
              // {
                cargoExtraArgs = "--workspace";
              }
            );

            coverage-tests = craneLib.cargoLlvmCov (
              commonCheckArgs
              // {
                pname = "wasm-trampoline-coverage-tests";
                inherit cargoArtifacts;
                cargoLlvmCovCommand = "nextest";
                cargoLlvmCovExtraArgs = "--ignore-filename-regex 'nix/store' --workspace --cobertura --output-path $out";
              }
            );
          };
        };
    };
}
