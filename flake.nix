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
        };
    };
}
