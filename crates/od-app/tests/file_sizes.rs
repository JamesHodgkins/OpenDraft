//! ARCHITECTURE.md rule 5: files stay under about 400 lines of production
//! code (everything before the `mod tests` block). Files already over the
//! limit are listed with their current size and may not grow; split them and
//! remove them from the list.

use std::fs;
use std::path::{Path, PathBuf};

const LIMIT: usize = 450;

/// Known over the limit: (path from the repository root, most lines allowed).
const TO_SPLIT: &[(&str, usize)] = &[("crates/od-files/src/dxf/read.rs", 425)];

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

fn production_lines(text: &str) -> usize {
    let end = text
        .find("#[cfg(test)]\nmod tests")
        .or_else(|| text.find("#[cfg(test)]\r\nmod tests"))
        .unwrap_or(text.len());
    text[..end].lines().count()
}

#[test]
fn files_stay_within_the_size_rule() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    sources(&root.join("crates"), &mut files);
    assert!(files.len() > 50, "found the source tree");

    let mut problems = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/")
            .replace("/../..", "");
        let relative = relative.trim_start_matches("../../").to_owned();
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let lines = production_lines(&text);
        let allowed = TO_SPLIT
            .iter()
            .find(|(name, _)| relative.ends_with(name))
            .map_or(LIMIT, |&(_, cap)| cap);
        if lines > allowed {
            problems.push(format!("{relative}: {lines} lines (limit {allowed})"));
        }
    }
    assert!(problems.is_empty(), "too long:\n{}", problems.join("\n"));
}
