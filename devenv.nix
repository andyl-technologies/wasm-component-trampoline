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
  ];

  languages.rust.enable = true;

  processes =
    {
    }
    // lib.optionalAttrs (config.devenv.isTesting) {
    }
    // lib.optionalAttrs (!config.devenv.isTesting) {
      cargo-watch.exec = "cargo-watch";
    };

  enterTest = ''
    cargo test --workspace
    cargo fmt --check --all
  '';

  git-hooks.hooks.nixfmt-rfc-style.enable = true;
}
