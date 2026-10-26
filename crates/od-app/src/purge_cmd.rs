//! PURGE: delete what the drawing no longer uses. PURGE opens the dialog
//! (`app/purge_dialog.rs`), which lists `od_core::purge::purgeable` and
//! deletes the ticked items; `-PURGE` deletes everything, repeating while
//! that frees more. Each is one undo step.

use od_commands::{
    Cancelled, Editor, RemoveLayer, SetBlocks, SetDimStyles, SetLinetypes, SetMLeaderStyles,
    SetTextStyles, Transaction,
};
use od_core::purge::{purgeable, Category, Item, Purgeable};

use crate::text::plural;

/// The Purge dialog while it is open.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    /// What can go, and which of it is ticked.
    pub items: Vec<(Purgeable, bool)>,
    /// A purge was started: list again once it is done.
    pub stale: bool,
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

/// An edit worked out while the drawing was borrowed, made once it is not.
type Step = Box<dyn FnOnce(&mut Transaction)>;

/// Delete `items` now, as part of `tx`.
fn delete(tx: &mut Transaction, items: &[Item]) {
    let doc = tx.document();
    let has = |f: &dyn Fn(&Item) -> bool| items.iter().any(f);
    let mut actions: Vec<Step> = Vec::new();
    if has(&|i| matches!(i, Item::Block(_))) {
        let kept = doc
            .blocks()
            .iter()
            .filter(|b| !items.contains(&Item::Block(b.id)))
            .cloned()
            .collect();
        actions.push(Box::new(move |tx| {
            tx.perform(SetBlocks::new("Purge", kept))
        }));
    }
    if has(&|i| matches!(i, Item::DimStyle(_))) {
        let kept = doc
            .dim_styles()
            .iter()
            .filter(|s| !items.contains(&Item::DimStyle(s.name.clone())))
            .cloned()
            .collect();
        let current = doc.dim_style().clone();
        actions.push(Box::new(move |tx| {
            tx.perform(SetDimStyles::new("Purge", kept, current, Vec::new()))
        }));
    }
    for item in items {
        if let Item::Layer(id) = item {
            let id = *id;
            actions.push(Box::new(move |tx| tx.perform(RemoveLayer::new(id))));
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
        let scale = doc.linetype_scale();
        actions.push(Box::new(move |tx| {
            tx.perform(SetLinetypes::new(table, scale))
        }));
    }
    if has(&|i| matches!(i, Item::MLeaderStyle(_))) {
        let kept = doc
            .mleader_styles()
            .iter()
            .filter(|s| !items.contains(&Item::MLeaderStyle(s.name.clone())))
            .cloned()
            .collect();
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        actions.push(Box::new(move |tx| {
            tx.perform(SetMLeaderStyles::new(
                "Purge",
                kept,
                current,
                next,
                Vec::new(),
            ))
        }));
    }
    if has(&|i| matches!(i, Item::TextStyle(_))) {
        let kept = doc
            .text_styles()
            .iter()
            .filter(|s| !items.contains(&Item::TextStyle(s.id)))
            .cloned()
            .collect();
        let (current, next) = (doc.current_text_style(), doc.next_text_style_id());
        actions.push(Box::new(move |tx| {
            tx.perform(SetTextStyles::new(kept, current, next))
        }));
    }
    let mut objects = Vec::new();
    for item in items {
        if let Item::ZeroLength(ids) | Item::EmptyText(ids) = item {
            objects.extend(ids.iter().copied());
        }
    }
    if !objects.is_empty() {
        actions.push(Box::new(move |tx| tx.remove(objects)));
    }
    for action in actions {
        action(tx);
    }
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

/// -PURGE: delete everything unused, and what that frees in turn.
pub fn purge_all(ed: &mut Editor) -> Result<(), Cancelled> {
    if ed.read(|doc| purgeable(doc).is_empty())? {
        ed.say("Nothing to purge.");
        return Ok(());
    }
    let gone = ed.transaction("Purge", |tx| {
        let mut gone = Vec::new();
        // Each pass can free what the one before still held; stop at a
        // pass that finds nothing (or a safe number of them).
        for _ in 0..16 {
            let found = purgeable(tx.document());
            if found.is_empty() {
                break;
            }
            let items: Vec<Item> = found.iter().map(|p| p.item.clone()).collect();
            delete(tx, &items);
            gone.extend(found);
        }
        Ok(gone)
    })?;
    report(ed, &gone);
    Ok(())
}

/// Delete just the items the dialog had ticked (those still purgeable).
pub fn purge_items(ed: &mut Editor, wanted: Vec<Item>) -> Result<(), Cancelled> {
    let gone = ed.transaction("Purge", move |tx| {
        let found: Vec<Purgeable> = purgeable(tx.document())
            .into_iter()
            .filter(|p| wanted.contains(&p.item))
            .collect();
        let items: Vec<Item> = found.iter().map(|p| p.item.clone()).collect();
        delete(tx, &items);
        Ok(found)
    })?;
    report(ed, &gone);
    Ok(())
}

#[cfg(test)]
#[path = "purge_cmd_tests.rs"]
mod tests;
