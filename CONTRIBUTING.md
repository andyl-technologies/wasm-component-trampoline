# Contributing to the Project

Thank you for your interest in contributing to wasm-component-trampoline.
We welcome contributions from the community and appreciate your efforts to improve the codebase.

## Pre-requisite Knowledge

Before you start contributing, please ensure you have the following:
1. A working knowledge of Rust and [WebAssembly](https://webassembly.org/) (or a willingness to learn).
1. Familiarity with [Wasmtime](https://docs.wasmtime.dev/) as it relates to the [WebAssembly component model](https://component-model.bytecodealliance.org/).

## How to Develop

We use [devenv](https://devenv.sh/) to manage our development environment.

1. **Clone the Repository**: You know how to do that.
1. **Install direnv**: Follow the instructions at [direnv Installation](https://direnv.net/docs/installation.html) to install direnv.
1. **Install Devenv**: Follow the instructions at [Devenv Installation](https://devenv.sh/docs/installation) to install devenv.
1. `direnv allow $PWD` and/or `devenv shell` to enter the development environment.
1. `devenv test` to run all the tests locally.
1. (OPTIONAL) `devenv shell wasm-trampoline-coverage` to generate code coverage. `cargo llvm-cov  report --html --open --release` to view a coverage report in your browser.

## License

[License](LICENSE)

## Contribution Agreement

[Developer Certificate of Origin](https://developercertificate.org/)
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
