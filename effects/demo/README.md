# Pixel sweep demo effect

This `no_std` Rust crate builds the WebAssembly module consumed by the firmware's
ABI v1 effect host. The effect lights one logical pixel at a time, advancing
through the host's fixture-then-span pixel order every 250 ms.

The demo supports up to 4096 logical pixels so its static framebuffer fits
within the host's 64 KiB Wasm memory limit. `bled-core` is used to decode the
CBOR device config supplied by the host and verify its logical pixel count.
