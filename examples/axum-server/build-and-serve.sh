#!/usr/bin/env bash

set -ex

adapter_file=wasi_snapshot_preview1.reactor.wasm
adapter_url='https://github.com/bytecodealliance/wasmtime/releases/download/v15.0.0/wasi_snapshot_preview1.reactor.wasm'
[ -e $adapter_file ] || curl -L $adapter_url -o $adapter_file

cargo build --release

wasm-tools component new \
	target/wasm32-wasi/release/axum_server.wasm \
	--adapt $adapter_file \
	-o server.wasm

wasmtime serve server.wasm
