#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Reads the ezdxf attribute sample and writes it back as DXF (R2000), for
//! `tests/fixtures/check_attributes.py` and for opening in AutoCAD.
//!
//!     cargo run -p od-files --example export_attributes -- out.dxf

use std::path::Path;

use od_files::{DxfFormat, FileFormat};

fn main() {
    let out = std::env::args().nth(1).unwrap();
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_attributes.dxf");
    let loaded = od_files::load(&sample).unwrap();
    let encoded = DxfFormat.write(&loaded.document).unwrap();
    std::fs::write(&out, encoded.bytes).unwrap();
    for note in encoded.notes {
        println!("note: {note}");
    }
    println!("wrote {out}");
}
