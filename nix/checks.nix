{ ... }:
{
  perSystem =
    {
      config,
      pkgs,
      craneOutputs,
      ...
    }:
    {
      pre-commit = {
        check.enable = true;
        settings.hooks = {
          actionlint.enable = true;
          nixfmt-rfc-style.enable = true;
        };
      };

      # Expose crane checks via `nix flake check`
      checks = {
        # Rust formatting
        cargo-fmt = craneOutputs.fmt;
        # Clippy lints
        cargo-clippy = craneOutputs.clippy;
        # Tests via nextest
        cargo-nextest = craneOutputs.nextest;
        # Build the crate
        cargo-build = craneOutputs.crate;
      };
    };
}
