//! Every dialog is built with `od_ui::widgets::dialog`, which lays its header
//! and close button out to the window's real width. A dialog that draws its
//! own title bar or close button can put that button short of the corner
//! (it did, once), so nothing outside the widget may.

use std::fs;
use std::path::{Path, PathBuf};

/// The only place that draws a dialog's title bar and close button.
const THE_WIDGET: &str = "crates/od-ui/src/widgets/containers.rs";

/// What only the widget may use: a window with no title bar of its own to
/// replace, and the close button's icon.
const CHROME: &[&str] = &[".title_bar(false)", "DIALOG_CLOSE"];

/// Files that name these without drawing a dialog (the icon table itself).
const ALLOWED: &[&str] = &[
    "crates/od-ui/src/icons.rs",
    "crates/od-app/tests/dialog_chrome.rs",
];

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

#[test]
fn only_the_dialog_widget_draws_a_title_bar_or_close_button() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    sources(&root.join("crates"), &mut files);
    let mut offenders = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if relative == THE_WIDGET || ALLOWED.contains(&relative.as_str()) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for word in CHROME {
            if text.contains(word) {
                offenders.push(format!("{relative}: {word}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "build dialogs with od_ui::widgets::dialog, not their own title bar or close button:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_widget_is_where_this_test_expects_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = fs::read_to_string(root.join(THE_WIDGET)).expect("the dialog widget's file");
    assert!(text.contains("pub fn dialog"));
    assert!(CHROME.iter().all(|word| text.contains(word)));
}
