{ ... }:
{
  perSystem =
    { pkgs, rustToolchains, ... }:
    {
      apps = {
        test = {
          type = "app";
          meta.description = "Run full test suite (fmt, check, nextest, WASM builds)";
          program = toString (
            pkgs.writeShellScript "test" ''
              set -e
              export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER=lld
              export CARGO_TARGET_WASM32_WASIP2_LINKER=lld
              cargo fmt --check --all
              cargo check --workspace --all-targets
              cargo nextest run --workspace
              for target in wasm32-unknown-unknown wasm32-wasip2; do
                cargo build --release --workspace --target $target
              done
              tests/runner/build.sh
            ''
          );
        };

        miri-test = {
          type = "app";
          meta.description = "Run memory safety tests with miri";
          program = toString (
            pkgs.writeShellScript "miri-test" ''
              set -ex
              # Use nightly toolchain from fenix
              export PATH="${rustToolchains.nightly}/bin:$PATH"
              cargo miri setup
              cargo miri nextest run --workspace
            ''
          );
        };

        coverage = {
          type = "app";
          meta.description = "Generate coverage reports (cobertura XML and lcov)";
          program = toString (
            pkgs.writeShellScript "coverage" ''
              set -e
              tests/runner/build.sh >/dev/null
              cargo llvm-cov clean --workspace
              cargo llvm-cov nextest --workspace --no-report --release
              cargo llvm-cov run --bin runner -p runner --release --no-report
              cargo llvm-cov run --bin async-runner -p runner --release --no-report
              cargo llvm-cov report --release --cobertura --output-path coverage.cobertura.xml
              cargo llvm-cov report --release --lcov --output-path coverage.lcov
            ''
          );
        };
      };
    };
}
