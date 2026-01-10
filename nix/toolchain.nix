{ inputs, ... }:
{
  perSystem =
    { system, ... }:
    let
      fenixPkgs = inputs.fenix.packages.${system};
    in
    {
      # Export toolchains for use in other modules
      _module.args.rustToolchains = {
        # Stable Rust with WASM targets
        stable = fenixPkgs.combine [
          fenixPkgs.stable.cargo
          fenixPkgs.stable.clippy
          fenixPkgs.stable.llvm-tools-preview
          fenixPkgs.stable.rust-analyzer
          fenixPkgs.stable.rustc
          fenixPkgs.stable.rustfmt
          fenixPkgs.stable.rust-src
          fenixPkgs.targets.wasm32-unknown-unknown.stable.rust-std
          fenixPkgs.targets.wasm32-wasip2.stable.rust-std
        ];

        # Nightly with miri for testing (pure fenix, no rustup)
        nightly = fenixPkgs.combine [
          fenixPkgs.complete.cargo
          fenixPkgs.complete.rustc
          fenixPkgs.complete.rust-src
          fenixPkgs.complete.miri
        ];
      };
    };
}
