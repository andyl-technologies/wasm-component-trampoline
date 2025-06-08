WASM Component Trampoline
=========================

[![Crates.io](https://img.shields.io/crates/v/wasm-component-trampoline.svg)](https://crates.io/crates/wasm-component-trampoline)
[![Documentation](https://docs.rs/wasm-component-trampoline/badge.svg)](https://docs.rs/wasm-component-trampoline)
![License](https://img.shields.io/crates/l/wasm-component-trampoline.svg)

Library for linking WASM components together using host "trampoline" functions, that can securely read/modify the host
context between component calls. Designed for WIT (WASM Interface Types) components, but can be used with others.

![WASM Component Trampoline Example Diagram](https://raw.githubusercontent.com/andyl-technologies/wasm-component-trampoline/refs/heads/master/docs/images/example_diagram.svg)

## Installation

```shell
cargo add wasm-component-trampoline
```

## Usage

 - [Sync WASM runtime example](https://github.com/andyl-technologies/wasm-component-trampoline/blob/master/tests/runner/src/bin/runner.rs)
 - [Async WASM runtime example](https://github.com/andyl-technologies/wasm-component-trampoline/blob/master/tests/runner/src/bin/async-runner.rs)
