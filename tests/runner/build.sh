#!/bin/bash
set -ex

TARGET_DIR=$(printf "%s/wasm32-unknown-unknown/release" "$(cargo metadata --no-deps --format-version 1 | jq -r '.target_directory')")
readonly WASM_TARGET_DIR="${TARGET_DIR}"

for x in kvstore logger application; do
	cargo build --target wasm32-unknown-unknown --release -p "$x"
	wasm-tools component new \
		"${WASM_TARGET_DIR}/$x.wasm" > \
		"${WASM_TARGET_DIR}/$x.component.wasm"
done

cargo run -p runner --bin runner --release
cargo run -p runner --bin async-runner --release
