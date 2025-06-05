WASM Component Trampoline
=========================

Library for linking WASM components together using host "trampoline" functions, that can securely read/modify the host
context between component calls. Designed for WIT (WASM Interface Types) components, but can be used with others.

![WASM Component Trampoline Example Diagram](/docs/images/example_diagram.svg)

## Installation

```shell
cargo install wasm-component-trampoline
```

## Usage

Sync WASM runtime example:
https://github.com/andyl-technologies/wasm-trampoline/blob/master/tests/runner/src/bin/runner.rs#L1-L200

Async WASM runtime example:
https://github.com/andyl-technologies/wasm-trampoline/blob/master/tests/runner/src/bin/async-runner.rs#L1-L200
