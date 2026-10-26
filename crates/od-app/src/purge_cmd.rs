//! PURGE: delete what the drawing no longer uses. PURGE opens the dialog
//! (`app/purge_dialog.rs`), which lists `od_core::purge::purgeable` and
//! deletes the ticked items; `-PURGE` deletes everything, repeating while
//! that frees more. Each is one undo step.

use od_commands::{
    Action, Cancelled, Editor, Options, RemoveEntities, RemoveLayer, SetBlocks, SetDimStyles,
    SetLinetypes, SetMLeaderStyles, SetTextStyles,
};
use od_core::purge::{purgeable, Category, Item, Purgeable};
use od_core::Document;

use crate::text::plural;

/// The Purge dialog while it is open.
pub struct Dialog {
    pub open: bool,
    /// Ask about each item first.
    pub confirm: bool,
    /// Go on to what deleting the items frees.
    pub nested: bool,
    /// What can go, and which of it is ticked.
    pub items: Vec<(Purgeable, bool)>,
    /// A purge was started: list again once it is done.
    pub stale: bool,
}

impl Default for Dialog {
    fn default() -> Self {
        Dialog {
            open: false,
            confirm: false,
            nested: true,
            items: Vec::new(),
            stale: false,
        }
    }
}

impl crate::state::AppState {
    /// Open the Purge dialog on what can go now, all of it ticked.
    pub fn open_purge(&mut self) {
        self.refresh_purge();
        self.purge.open = true;
    }

    /// List what can go again, keeping the ticks of what is still there.
    pub fn refresh_purge(&mut self) {
        let found = purgeable(self.session().document());
        let old = std::mem::take(&mut self.purge.items);
        self.purge.items = found
            .into_iter()
            .map(|p| {
                let ticked = old
                    .iter()
                    .find(|(o, _)| o.item == p.item)
                    .is_none_or(|o| o.1);
                (p, ticked)
            })
            .collect();
        self.purge.stale = false;
    }
}

/// The edits that delete `items` from `doc`, worked out from it.
fn delete_actions(doc: &Document, items: &[Item]) -> Vec<Box<dyn Action>> {
    let has = |f: &dyn Fn(&Item) -> bool| items.iter().any(f);
    let mut actions: Vec<Box<dyn Action>> = Vec::new();
    if has(&|i| matches!(i, Item::Block(_))) {
        let kept = doc
            .blocks()
            .iter()
            .filter(|b| !items.contains(&Item::Block(b.id)))
            .cloned()
            .collect();
        actions.push(Box::new(SetBlocks::new("Purge", kept)));
    }
    if has(&|i| matches!(i, Item::DimStyle(_))) {
        let kept = doc
            .dim_styles()
            .iter()
            .filter(|s| !items.contains(&Item::DimStyle(s.name.clone())))
            .cloned()
            .collect();
        let current = doc.dim_style().clone();
        actions.push(Box::new(SetDimStyles::new(
            "Purge",
            kept,
            current,
            Vec::new(),
        )));
    }
    for item in items {
        if let Item::Layer(id) = item {
            actions.push(Box::new(RemoveLayer::new(*id)));
        }
    }
    if has(&|i| matches!(i, Item::Linetype(_))) {
        let mut table = doc.linetypes().clone();
        for item in items {
            if let Item::Linetype(name) = item {
                if let Some(id) = table.named(name).map(|r| r.id) {
                    // Special records are never offered; ignore a refusal.
                    let _ = table.remove(id);
                }
            }
        }
        actions.push(Box::new(SetLinetypes::new(table, doc.linetype_scale())));
    }
    if has(&|i| matches!(i, Item::MLeaderStyle(_))) {
        let kept = doc
            .mleader_styles()
            .iter()
            .filter(|s| !items.contains(&Item::MLeaderStyle(s.name.clone())))
            .cloned()
            .collect();
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        actions.push(Box::new(SetMLeaderStyles::new(
            "Purge",
            kept,
            current,
            next,
            Vec::new(),
        )));
    }
    if has(&|i| matches!(i, Item::TextStyle(_))) {
        let kept = doc
            .text_styles()
            .iter()
            .filter(|s| !items.contains(&Item::TextStyle(s.id)))
            .cloned()
            .collect();
        let (current, next) = (doc.current_text_style(), doc.next_text_style_id());
        actions.push(Box::new(SetTextStyles::new(kept, current, next)));
    }
    let mut objects = Vec::new();
    for item in items {
        if let Item::ZeroLength(ids) | Item::EmptyText(ids) = item {
            objects.extend(ids.iter().copied());
        }
    }
    if !objects.is_empty() {
        actions.push(Box::new(RemoveEntities::new(objects)));
    }
    actions
}

/// What was deleted, one line for each, then the total.
fn report(ed: &Editor, gone: &[Purgeable]) {
    for p in gone {
        let what = match p.item.category() {
            Category::Blocks => format!("block \"{}\"", p.name),
            Category::DimStyles => format!("dimension style \"{}\"", p.name),
            Category::Layers => format!("layer \"{}\"", p.name),
            Category::Linetypes => format!("linetype \"{}\"", p.name),
            Category::MLeaderStyles => format!("multileader style \"{}\"", p.name),
            Category::TextStyles => format!("text style \"{}\"", p.name),
            Category::ZeroLength => format!("{} of zero-length geometry", p.name),
            Category::EmptyText => format!("{} of empty text", p.name),
        };
        ed.say(format!("Deleting {what}."));
    }
    ed.say(format!("{} purged.", plural(gone.len(), "item")));
}

/// What a purge may do: which items (all when `None`), whether to go on to
/// what deleting them frees, and whether to ask about each one first.
#[derive(Clone, Default)]
pub struct Purge {
    pub only: Option<Vec<Item>>,
    pub nested: bool,
    pub confirm: bool,
}

/// Ask about one item: Yes, No, or All (yes to this and the rest).
fn confirmed(ed: &Editor, p: &Purgeable, all: &mut bool) -> Result<bool, Cancelled> {
    if *all {
        return Ok(true);
    }
    let what = p.item.category().title();
    let options = Options::<String>::new(format!("Purge {what} \"{}\"? [Yes/No/All]", p.name))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .keyword("All", "A")
        .default("Yes".to_owned());
    Ok(match ed.ask_keyword(options)?.as_deref() {
        Some("No") => false,
        Some("All") => {
            *all = true;
            true
        }
        _ => true,
    })
}

/// PURGE and -PURGE: delete what `purge` says. The questions come first and
/// change nothing; then everything is deleted as one undo step. With nested
/// items, each round is worked out on a copy of the drawing to find what
/// deleting it frees.
pub fn purge_run(ed: &mut Editor, purge: Purge) -> Result<(), Cancelled> {
    let mut sim = ed.read(|doc| doc.clone())?;
    let mut found = purgeable(&sim);
    let mut seen: Vec<Item> = found.iter().map(|p| p.item.clone()).collect();
    if let Some(only) = &purge.only {
        found.retain(|p| only.contains(&p.item));
    }
    let (mut rounds, mut all) = (Vec::<Vec<Purgeable>>::new(), false);
    while !found.is_empty() {
        let mut accepted = Vec::new();
        for p in found {
            if !purge.confirm || confirmed(ed, &p, &mut all)? {
                accepted.push(p);
            }
        }
        if accepted.is_empty() {
            break;
        }
        let items: Vec<Item> = accepted.iter().map(|p| p.item.clone()).collect();
        for mut action in delete_actions(&sim, &items) {
            action.apply(&mut sim);
        }
        rounds.push(accepted);
        if !purge.nested || rounds.len() >= 16 {
            break;
        }
        found = purgeable(&sim)
            .into_iter()
            .filter(|p| !seen.contains(&p.item))
            .collect();
        seen.extend(found.iter().map(|p| p.item.clone()));
    }
    if rounds.is_empty() {
        ed.say("Nothing to purge.");
        return Ok(());
    }
    let replay = rounds.clone();
    ed.transaction("Purge", move |tx| {
        for round in &replay {
            let items: Vec<Item> = round.iter().map(|p| p.item.clone()).collect();
            for action in delete_actions(tx.document(), &items) {
                tx.perform(action);
            }
        }
        Ok(())
    })?;
    report(ed, &rounds.concat());
    Ok(())
}

/// -PURGE: delete everything unused, and what that frees in turn.
pub fn purge_all(ed: &mut Editor) -> Result<(), Cancelled> {
    purge_run(
        ed,
        Purge {
            only: None,
            nested: true,
            confirm: false,
        },
    )
}

#[cfg(test)]
#[path = "purge_cmd_tests.rs"]
mod tests;
