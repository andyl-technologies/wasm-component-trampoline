rustToolchain: _final: prev:
let
  inherit (prev) lib;
in
(lib.optionalAttrs prev.stdenv.isDarwin {
  cargo-llvm-cov =
    (prev.cargo-llvm-cov.override {
      rustPlatform = prev.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    }).overrideAttrs
      (o: {
        # Some coverage tests are sporadically broken on macOS.
        # https://github.com/NixOS/nixpkgs/blob/90293bb42f080c84fdfaae86336a87a8aa996638/pkgs/by-name/ca/cargo-llvm-cov/package.nix#L1-L14
        doCheck = false;

        meta.broken = false;
      });
})
