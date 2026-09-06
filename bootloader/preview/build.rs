//! The bootloader's own atlas generator, plus the dependency tracking cargo
//! will not do for us.
//!
//! The generator is included rather than copied, so the preview cannot render
//! text from a different atlas than the one the firmware will. `find_font`
//! walks up from `CARGO_MANIFEST_DIR`, which is why it still finds the font
//! from one directory deeper.

#[path = "../build.rs"]
mod generator;

use std::path::Path;

fn main() {
    generator::main();

    // The modules this crate previews live outside its own directory, reached
    // by `#[path]`. Cargo's change detection does not follow that: editing
    // `../src/menu.rs` leaves the preview binary stale, and it re-renders the
    // previous layout while reporting success — which is worse than not
    // building at all, because the PNG looks like an answer.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
    let Ok(entries) = std::fs::read_dir(&src) else {
        panic!("cannot read {}", src.display());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
