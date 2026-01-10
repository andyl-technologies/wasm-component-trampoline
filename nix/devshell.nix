# Development shells using crane's devShell helper
{ inputs, ... }:
{
  perSystem =
    {
      config,
      pkgs,
      rustToolchains,
      craneOutputs,
      ...
    }:
    {
      devShells = {
        # Default shell using crane's devShell with additional tools
        default = craneOutputs.craneLib.devShell {
          # Include common build inputs from crane
          inputsFrom = [ craneOutputs.crate ];

          # Additional development tools
          packages = [
            pkgs.cargo-nextest
            pkgs.cargo-vet
            pkgs.cargo-watch
            pkgs.cargo-llvm-cov
            pkgs.git
            pkgs.jq
            pkgs.wasm-tools
            pkgs.wasmtime
          ];

          # For rust-analyzer
          RUST_SRC_PATH = "${rustToolchains.stable}/lib/rustlib/src/rust/library";

          shellHook = ''
            ${config.pre-commit.installationScript}
            echo "wasm-component-trampoline dev shell (crane)"
          '';
        };

        # Separate shell for miri testing
        miri = pkgs.mkShell {
          name = "wasm-component-trampoline-miri";
          nativeBuildInputs = [
            rustToolchains.nightly
            pkgs.cargo-nextest
          ];
          shellHook = ''
            echo "Miri shell - run: cargo miri nextest run --workspace"
          '';
        };
      };
    };
}
