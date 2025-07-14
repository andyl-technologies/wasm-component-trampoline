{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";

    flake-parts.url = "github:hercules-ci/flake-parts";

    crane.url = "github:ipetkov/crane";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        {
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

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          commonCheckArgs = commonArgs // {
            inherit cargoArtifacts;
            doCheck = true;
          };
        in
        {
          _module.args = {
            inherit craneLib;
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

          checks = {
            format = craneLib.cargoFmt (
              commonCheckArgs
              // {
                cargoExtraArgs = "--all";
              }
            );
          };
        };
    };
}
