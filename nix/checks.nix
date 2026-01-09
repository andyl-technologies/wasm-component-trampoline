{ ... }:
{
  perSystem =
    { config, pkgs, ... }:
    {
      pre-commit = {
        check.enable = true;
        settings.hooks = {
          actionlint.enable = true;
          nixfmt-rfc-style.enable = true;
        };
      };
    };
}
