//! Writes the table of icon drawings built into the program: every `.svg` in
//! `assets/icons`, sorted by name (see `src/icon_set.rs`).

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("../../assets/icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".svg"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut table = String::from("pub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for name in &names {
        let path = dir.join(name).to_string_lossy().replace('\\', "/");
        let _ = writeln!(table, "    ({name:?}, include_bytes!({path:?})),");
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("embedded_icons.rs");
    if let Err(e) = std::fs::write(&out, table) {
        println!("cargo:warning=could not write {}: {e}", out.display());
    }
}
