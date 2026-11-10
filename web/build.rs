use std::fs;
use std::path::Path;

fn main() {
    let dest_path = Path::new(".tailwind");
    let _ = fs::write(dest_path, leptodon::include_generated::all());
    println!("cargo::rerun-if-changed=Cargo.toml");
}
