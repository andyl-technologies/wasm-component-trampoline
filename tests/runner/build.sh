#!/bin/bash
set -ex

cargo build --target wasm32-unknown-unknown --release --workspace -p wasm-component-trampoline -p runner

for x in kvstore logger application; do
  wasm-tools component new \
    target/wasm32-unknown-unknown/release/$x.wasm > target/wasm32-unknown-unknown/release/$x.component.wasm
done

cargo run -p runner --bin runner --release
cargo run -p runner --bin async-runner --release
