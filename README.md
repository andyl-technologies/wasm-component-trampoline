# WebAssembly Component Trampoline

[![Crates.io](https://img.shields.io/crates/v/wasm-component-trampoline.svg)](https://crates.io/crates/wasm-component-trampoline)
[![Documentation](https://docs.rs/wasm-component-trampoline/badge.svg)](https://docs.rs/wasm-component-trampoline)
![License](https://img.shields.io/crates/l/wasm-component-trampoline.svg)

Library for linking WebAssembly components together using host "trampoline" functions.
Trampoline functions can read and modify the provided context between inter-component calls.
Guests will not directly call other components, but must go through the host trampoline functions.

Specifically designed for versioned [WIT](https://component-model.bytecodealliance.org/design/wit.html)
(WASM Interface Type) packages, but users can intercept any components calls.
Versioned dependency resolution occurs between components in the same style as the Wasmtime component linker
([docs](https://docs.wasmtime.dev/api/wasmtime/component/struct.Linker.html#names-and-semver)).

![Trampoline Example Diagram](https://raw.githubusercontent.com/andyl-technologies/wasm-component-trampoline/refs/heads/master/docs/images/example_diagram.svg)

## Installation

```shell
cargo add wasm-component-trampoline
```

## Usage

- [Sync WASM runtime example](https://github.com/andyl-technologies/wasm-component-trampoline/blob/master/tests/runner/src/bin/runner.rs)
- [Async WASM runtime example](https://github.com/andyl-technologies/wasm-component-trampoline/blob/master/tests/runner/src/bin/async-runner.rs)
