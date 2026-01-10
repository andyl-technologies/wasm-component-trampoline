{ ... }:
{
  perSystem =
    { pkgs, rustToolchains, ... }:
    let
      commonDeps = [
        rustToolchains.stable
        pkgs.cargo-nextest
        pkgs.wasm-tools
        pkgs.wasmtime
        pkgs.llvmPackages.bintools
      ];
    in
    {
      apps = {
        test = {
          type = "app";
          meta.description = "Run full test suite (fmt, check, nextest, WASM builds)";
          program = "${
            pkgs.writeShellApplication {
              name = "test";
              runtimeInputs = commonDeps;
              text = ''
                export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER=lld
                export CARGO_TARGET_WASM32_WASIP2_LINKER=lld
                cargo fmt --check --all
                cargo check --workspace --all-targets
                cargo nextest run --workspace
                for target in wasm32-unknown-unknown wasm32-wasip2; do
                  cargo build --release --workspace --target "$target"
                done
                tests/runner/build.sh
              '';
            }
          }/bin/test";
        };

        miri-test = {
          type = "app";
          meta.description = "Run memory safety tests with miri";
          program = "${
            pkgs.writeShellApplication {
              name = "miri-test";
              runtimeInputs = [
                rustToolchains.nightly
                pkgs.cargo-nextest
              ];
              text = ''
                cargo miri setup
                cargo miri nextest run --workspace
              '';
            }
          }/bin/miri-test";
        };

        coverage = {
          type = "app";
          meta.description = "Generate coverage reports (cobertura XML and lcov)";
          program = "${
            pkgs.writeShellApplication {
              name = "coverage";
              runtimeInputs = commonDeps ++ [ pkgs.cargo-llvm-cov ];
              text = ''
                export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER=lld
                export CARGO_TARGET_WASM32_WASIP2_LINKER=lld
                WASM_DIR="target/wasm32-unknown-unknown/release"
                tests/runner/build.sh >/dev/null
                cargo llvm-cov clean --workspace
                cargo llvm-cov nextest --workspace --no-report --release
                cargo llvm-cov run --bin runner -p runner --release --no-report -- --wasm-dir "$WASM_DIR"
                cargo llvm-cov run --bin async-runner -p runner --release --no-report -- --wasm-dir "$WASM_DIR"
                cargo llvm-cov report --release --cobertura --output-path coverage.cobertura.xml
                cargo llvm-cov report --release --lcov --output-path coverage.lcov
              '';
            }
          }/bin/coverage";
        };
      };
    };
}
