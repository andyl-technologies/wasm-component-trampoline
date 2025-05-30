{
  pkgs,
  config,
  lib,
  ...
}:

{
  packages = with pkgs; [
    cargo-watch
    git
    lld
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
    cargo test --workspace
    cargo build --workspace --target wasm32-unknown-unknown
    cargo build --workspace --target wasm32-wasip2
    cargo fmt --check --all
    tests/runner/build.sh
    cargo llvm-cov  run --bin runner -p runner --release
  '';

  git-hooks.hooks.nixfmt-rfc-style.enable = true;
  git-hooks.hooks.actionlint.enable = true;
}
