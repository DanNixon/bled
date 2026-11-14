name := replace(file_name(justfile_directory()), "-", "_")
wasm := "target/wasm32-unknown-unknown/release/" + name + ".wasm"

default: check

build:
    cargo build --release --target wasm32-unknown-unknown
    wasm-opt -Oz --strip-debug {{wasm}} -o {{wasm}}
    ls -l {{wasm}}

fmt:
    cargo fmt

clippy:
    cargo clippy --target wasm32-unknown-unknown -- -D warnings

check: clippy
    cargo fmt --check
