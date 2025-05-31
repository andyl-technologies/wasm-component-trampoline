#!/bin/bash
set -ex

cargo build --target wasm32-unknown-unknown --release --workspace

for x in kvstore logger application; do
  wasm-tools component new \
    target/wasm32-unknown-unknown/release/$x.wasm > target/wasm32-unknown-unknown/release/$x.component.wasm
done

cargo run -p runner --release
