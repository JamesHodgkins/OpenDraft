//! The font families installed on this machine, for the Text Style dialog's
//! font list. Scanning the font folders takes a moment, so it runs on its own
//! thread from launch; until it finishes the list is empty.

use std::sync::OnceLock;

static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();

/// Start the scan; calling it again does nothing.
pub fn start_scan() {
    if FAMILIES.get().is_some() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("system-fonts".into())
        .spawn(|| {
            let _ = FAMILIES.set(scan());
        });
}

/// The installed families, sorted; empty until the scan has finished.
pub fn families() -> &'static [String] {
    FAMILIES.get().map_or(&[], Vec::as_slice)
}

fn scan() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_gives_sorted_unique_names() {
        let names = scan();
        for pair in names.windows(2) {
            assert!(pair[0].to_lowercase() < pair[1].to_lowercase());
        }
    }
}
