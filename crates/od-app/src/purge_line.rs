//! -PURGE: AutoCAD's command-line form of PURGE. It asks which kind of
//! unused object, then which names, then whether to verify each one, and
//! removes one level (run it again to take what that freed).

use od_commands::{Answer, Cancelled, Editor, Options};
use od_core::purge::{blocked, purgeable, Category, Item};

use crate::power_select::wildcard_match;
use crate::purge_cmd::{purge_run, Purge};

/// AutoCAD's other kinds, which OpenDraft has nothing of to purge.
const UNAVAILABLE: [(&str, &str, &str); 10] = [
    ("Groups", "G", "groups"),
    ("MAterials", "MA", "materials"),
    ("PLotstyles", "PL", "plot styles"),
    ("SHapes", "SH", "shapes"),
    ("Mlinestyles", "ML", "multiline styles"),
    ("Tablestyles", "TA", "table styles"),
    ("Visualstyles", "V", "visual styles"),
    ("Regapps", "R", "registered applications"),
    ("detail View styles", "DE", "detail view styles"),
    ("orphaned Data", "O", "orphaned DGN linestyle data"),
];

/// The named kinds, with the label and key each answers to.
const KINDS: [(Category, &str, &str); 6] = [
    (Category::Blocks, "Blocks", "B"),
    (Category::DimStyles, "Dimstyles", "D"),
    (Category::Layers, "LAyers", "LA"),
    (Category::Linetypes, "LTypes", "LT"),
    (Category::MLeaderStyles, "MUltileaderstyles", "MU"),
    (Category::TextStyles, "textSTyles", "ST"),
];

/// The items of `category` whose names match one of the comma-separated
/// `patterns`, saying why each match that cannot go stays.
fn matching(ed: &Editor, category: Category, patterns: &str) -> Result<Vec<Item>, Cancelled> {
    let (found, stays) = ed.read(|doc| (purgeable(doc), blocked(doc)))?;
    let mut items = Vec::new();
    for pattern in patterns.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let mut any = false;
        for p in found.iter().filter(|p| p.item.category() == category) {
            if wildcard_match(pattern, &p.name) {
                any = true;
                if !items.contains(&p.item) {
                    items.push(p.item.clone());
                }
            }
        }
        for b in stays.iter().filter(|b| b.item.category() == category) {
            if wildcard_match(pattern, &b.name) {
                any = true;
                ed.say(format!(
                    "\"{}\" cannot be purged: {}.",
                    b.name,
                    b.reasons.join("; ")
                ));
            }
        }
        if !any {
            ed.say(format!(
                "No {} match \"{pattern}\".",
                category.title().to_lowercase()
            ));
        }
    }
    Ok(items)
}

/// Whether to ask about each name first.
fn verify(ed: &Editor) -> Result<bool, Cancelled> {
    let options = Options::<String>::new("Verify each name to be purged? [Yes/No]")
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default("Yes".to_owned());
    Ok(ed.ask_keyword(options)?.as_deref() != Some("No"))
}

/// -PURGE.
pub fn purge_prompt(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut options = Options::<String>::new(
        "Enter type of unused objects to purge [Blocks/Dimstyles/LAyers/LTypes/MUltileaderstyles/textSTyles/Zero-length geometry/Empty text objects/All]",
    );
    for (_, label, key) in KINDS {
        options = options.keyword(label, key);
    }
    options = options
        .keyword("Zero-length geometry", "Z")
        .keyword("Empty text objects", "E")
        .keyword("All", "A");
    for (label, key, _) in UNAVAILABLE {
        options = options.keyword(label, key);
    }
    let choice = ed
        .ask_keyword(options.default("All".to_owned()))?
        .unwrap_or_else(|| "All".to_owned());
    if let Some((_, _, what)) = UNAVAILABLE.iter().find(|(label, ..)| *label == choice) {
        ed.say(format!(
            "There are no {what} to purge: OpenDraft does not store them."
        ));
        return Ok(());
    }
    let mut purge = Purge::default();
    match choice.as_str() {
        "Zero-length geometry" => {
            purge.only = Some(Vec::new());
            purge.zero_length = true;
        }
        "Empty text objects" => {
            purge.only = Some(Vec::new());
            purge.empty_text = true;
        }
        "All" => {}
        label => {
            let Some((category, ..)) = KINDS.iter().find(|(_, l, _)| *l == label) else {
                return Ok(());
            };
            let ask = Options::<String>::new("Enter name(s) to purge").default("*".to_owned());
            let names = match ed.ask_string(ask)? {
                Answer::Value(text) if !text.trim().is_empty() => text,
                _ => "*".to_owned(),
            };
            let items = matching(ed, *category, &names)?;
            if items.is_empty() {
                ed.say("Nothing to purge.");
                return Ok(());
            }
            purge.only = Some(items);
        }
    }
    purge.confirm = verify(ed)?;
    purge_run(ed, purge)
}
