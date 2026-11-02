#![allow(clippy::print_stderr, clippy::print_stdout)]
//! Runs OpenDraft's complete DWG import pipeline over every `.dwg` below a
//! directory, including conversion into an `od_core::Document`.
//!
//! Unsupported DWG versions are reported separately and do not make the run
//! fail. Read errors, importer errors for supported versions, and panics do.
//!
//!     cargo run -p od-files --example stress_dwg_import -- "private_material/autodesk samples"

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use od_files::{DwgFormat, FileError, FileFormat};

#[derive(Default)]
struct Totals {
    imported: usize,
    unsupported: usize,
    failed: usize,
    panicked: usize,
    objects: usize,
    warnings: usize,
    warning_kinds: BTreeMap<String, usize>,
    elapsed: Duration,
}

fn dwg_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file()
                && entry.path().extension().is_some_and(|extension| {
                    extension.to_string_lossy().eq_ignore_ascii_case("dwg")
                })
            {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn display_path<'a>(root: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

fn version(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.get(..6).unwrap_or(bytes)).into_owned()
}

fn main() {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: stress_dwg_import <directory>");
        std::process::exit(2);
    };
    let files = match dwg_files(&root) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("cannot scan {}: {error}", root.display());
            std::process::exit(2);
        }
    };
    if files.is_empty() {
        eprintln!("no DWG files found below {}", root.display());
        std::process::exit(2);
    }

    let mut totals = Totals::default();
    println!("status\tversion\tobjects\twarnings\tms\tfile");
    for path in &files {
        let started = Instant::now();
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                totals.failed += 1;
                eprintln!(
                    "FAIL\tread error: {error}\t{}",
                    display_path(&root, path).display()
                );
                continue;
            }
        };
        let source_version = version(&bytes);
        let result = std::panic::catch_unwind(|| DwgFormat.read(&bytes));
        let elapsed = started.elapsed();
        totals.elapsed += elapsed;
        match result {
            Ok(Ok(loaded)) => {
                totals.imported += 1;
                totals.objects += loaded.document.len();
                totals.warnings += loaded.warnings.len();
                for warning in &loaded.warnings {
                    *totals.warning_kinds.entry(warning.clone()).or_default() += 1;
                }
                println!(
                    "OK\t{}\t{}\t{}\t{}\t{}",
                    source_version,
                    loaded.document.len(),
                    loaded.warnings.len(),
                    elapsed.as_millis(),
                    display_path(&root, path).display()
                );
            }
            Ok(Err(FileError::Unsupported(error))) => {
                totals.unsupported += 1;
                println!(
                    "UNSUPPORTED\t{}\t-\t-\t{}\t{} ({})",
                    source_version,
                    elapsed.as_millis(),
                    display_path(&root, path).display(),
                    error
                );
            }
            Ok(Err(error)) => {
                totals.failed += 1;
                eprintln!(
                    "FAIL\t{}\t{}\t{}\t{}",
                    source_version,
                    elapsed.as_millis(),
                    display_path(&root, path).display(),
                    error
                );
            }
            Err(_) => {
                totals.panicked += 1;
                eprintln!(
                    "PANIC\t{}\t{}\t{}",
                    source_version,
                    elapsed.as_millis(),
                    display_path(&root, path).display()
                );
            }
        }
    }

    println!(
        "summary: {} files; {} imported, {} unsupported, {} failed, {} panicked; \
         {} objects, {} warning occurrences ({} kinds); {} ms",
        files.len(),
        totals.imported,
        totals.unsupported,
        totals.failed,
        totals.panicked,
        totals.objects,
        totals.warnings,
        totals.warning_kinds.len(),
        totals.elapsed.as_millis()
    );
    for (warning, files) in &totals.warning_kinds {
        println!("warning ({files} file(s)): {warning}");
    }
    if totals.failed != 0 || totals.panicked != 0 {
        std::process::exit(1);
    }
}
