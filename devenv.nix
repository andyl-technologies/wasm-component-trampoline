{ pkgs, lib, config, inputs, ... }:

{
    # https://devenv.sh/packages/
    packages = [ pkgs.git ];

    # https://devenv.sh/languages/
    languages.rust.enable = true;

    # https://devenv.sh/processes/
    processes.cargo-watch.exec = "cargo-watch";

    # https://devenv.sh/tests/
    enterTest = ''
        cargo test
        cargo fmt --check
    '';

    # See full reference at https://devenv.sh/reference/options/
}
