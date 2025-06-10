{
  pkgs,
  config,
  lib,
  ...
}:

{
  packages = with pkgs; [
    cargo-nextest
    cargo-watch
    git
    jq
    rustup
    wasm-tools
    wasmtime
  ];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
    targets = [
      "wasm32-unknown-unknown"
      "wasm32-wasip2"
      "wasm32v1-none"
    ];
    components = [
      "cargo"
      "clippy"
      "clippy-preview"
      "rust-analyzer"
      "rustc"
      "rustfmt"
      "llvm-tools-preview"
    ];
  };

  processes = lib.optionalAttrs (!config.devenv.isTesting) {
    cargo-watch.exec = "cargo-watch";
  };

  enterTest = ''
    set -ex
    cargo fmt --check --all
    cargo check --workspace --all-targets
    cargo nextest run --workspace
    for target in wasm32-unknown-unknown wasm32-wasip2; do
      cargo build --workspace --target ''${target}
    done
    build-runner
  '';

  git-hooks.hooks.actionlint.enable = true;
  git-hooks.hooks.nixfmt-rfc-style.enable = true;

  scripts."miri-test" = {
    description = "Run miri tests";
    exec = ''
      nix shell nixpkgs#rustup.out --command sh -c "
        set -ex
        rustup component add --toolchain nightly miri
        # cannot run wasi
        # cargo +nightly miri nextest run --target wasm32-wasip2 --workspace
        cargo +nightly miri setup
        cargo +nightly miri nextest run --workspace
      "
    '';
  };

  scripts."build-runner" = {
    description = "Build the wasm runner";
    exec = ''
      set -ex

      TARGET_DIR="$(
        cargo metadata --no-deps --format-version 1 | \
          ${pkgs.jq}/bin/jq -r '.target_directory' \
      )"
      readonly WASM_TARGET_DIR="''${TARGET_DIR}/wasm32-unknown-unknown/release"

      for x in kvstore logger application; do
        cargo build --target wasm32-unknown-unknown --release -p "$x"
        ${pkgs.wasm-tools}/bin/wasm-tools component new \
          "''${WASM_TARGET_DIR}/$x.wasm" > \
          "''${WASM_TARGET_DIR}/$x.component.wasm"
      done

      cargo run -p runner --bin runner --release
      cargo run -p runner --bin async-runner --release
    '';
  };

  scripts."wasm-trampoline-coverage" = {
    description = "Run wasm-trampoline-coverage";
    exec = ''
      build-runner
      cargo llvm-cov clean --workspace
      cargo llvm-cov nextest --workspace --no-report --release
      cargo llvm-cov run --bin runner -p runner --release --no-report
      cargo llvm-cov run --bin async-runner -p runner --release --no-report
      cargo llvm-cov report --release --cobertura --output-path coverage.cobertura.xml
      cargo llvm-cov report --release --lcov --output-path coverage.lcov
    '';
  };
}
