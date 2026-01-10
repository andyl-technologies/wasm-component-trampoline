# Crane configuration for building and testing the Rust workspace
{ inputs, ... }:
{
  perSystem =
    {
      pkgs,
      system,
      rustToolchains,
      ...
    }:
    let
      # Set up crane with fenix toolchain
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: rustToolchains.stable);

      # Source filtering - only include Rust-relevant files
      src = pkgs.lib.cleanSourceWith {
        src = inputs.self;
        filter =
          path: type:
          (craneLib.filterCargoSources path type)
          || (builtins.match ".*\\.wit$" path != null)
          || (builtins.match ".*/tests/wasm/.*" path != null);
      };

      # Common arguments shared across all crane derivations
      commonArgs = {
        inherit src;
        strictDeps = true;
        pname = "wasm-component-trampoline";
        # TODO: extract version from Cargo.toml
        version = "40.0.1-pre";

        # This crate only builds on Unix
        meta.platforms = pkgs.lib.platforms.unix;

        nativeBuildInputs = [
          pkgs.llvmPackages_18.bintools
        ];

        # Environment for WASM builds
        CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
        CARGO_TARGET_WASM32_WASIP2_LINKER = "lld";
      };

      # Build only dependencies for artifact caching/reuse
      # This is the key to crane's incremental builds
      cargoArtifacts = craneLib.buildDepsOnly (
        commonArgs
        // {
          pname = "wasm-component-trampoline-deps";
          # Build deps for all workspace members
          cargoExtraArgs = "--workspace";
        }
      );

      # Build the main crate
      crate = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoExtraArgs = "--workspace";
          # Skip tests here - we run them separately with nextest
          doCheck = false;
        }
      );

      # Run clippy
      clippy = craneLib.cargoClippy (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--workspace --all-targets --";
        }
      );

      # Run tests with nextest
      nextest = craneLib.cargoNextest (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoNextestExtraArgs = "--workspace";
          # nextest requires cargo-nextest in path
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.cargo-nextest ];
        }
      );

      # Generate code coverage with llvm-cov
      coverage = craneLib.cargoLlvmCov (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoLlvmCovExtraArgs = "--workspace --lcov --output-path $out/coverage.lcov";
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
            pkgs.cargo-llvm-cov
            pkgs.cargo-nextest
          ];
        }
      );

      # Check formatting
      fmt = craneLib.cargoFmt { inherit src; };

      # Build documentation
      doc = craneLib.cargoDoc (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoDocExtraArgs = "--workspace --no-deps";
        }
      );
    in
    {
      # Export crane artifacts for use in other modules
      _module.args.craneOutputs = {
        inherit
          craneLib
          src
          commonArgs
          cargoArtifacts
          crate
          clippy
          nextest
          coverage
          fmt
          doc
          ;
      };
    };
}
