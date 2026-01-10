{ inputs, ... }:
{
  perSystem =
    {
      config,
      pkgs,
      rustToolchains,
      ...
    }:
    {
      devShells = {
        default = pkgs.mkShell {
          name = "wasm-component-trampoline";

          nativeBuildInputs = [
            rustToolchains.stable
            pkgs.cargo-nextest
            pkgs.cargo-vet
            pkgs.cargo-watch
            pkgs.cargo-llvm-cov
            pkgs.git
            pkgs.jq
            pkgs.wasm-tools
            pkgs.wasmtime
            pkgs.llvmPackages.bintools # Required for WASM linking
          ];

          # Required for WASM builds on NixOS
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
          CARGO_TARGET_WASM32_WASIP2_LINKER = "lld";

          # For rust-analyzer
          RUST_SRC_PATH = "${rustToolchains.stable}/lib/rustlib/src/rust/library";

          shellHook = ''
            ${config.pre-commit.installationScript}
            echo "wasm-component-trampoline dev shell"
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
