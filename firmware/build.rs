use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let picow = env::var_os("CARGO_FEATURE_PICOW").is_some();
    let pico2w = env::var_os("CARGO_FEATURE_PICO2W").is_some();

    let (memory_file, memory) = match (picow, pico2w) {
        (false, false) => panic!("select a board feature: `picow` or `pico2w`"),
        (true, true) => panic!("board features `picow` and `pico2w` are mutually exclusive"),
        (true, false) => (
            "memory-picow.x",
            include_bytes!("memory-picow.x").as_slice(),
        ),
        (false, true) => (
            "memory-pico2w.x",
            include_bytes!("memory-pico2w.x").as_slice(),
        ),
    };

    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(memory)
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed={memory_file}");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
