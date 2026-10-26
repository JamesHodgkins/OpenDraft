//! PURGE: delete what the drawing no longer uses. PURGE opens the dialog
//! (`app/purge_dialog.rs`), which lists `od_core::purge::purgeable`, says
//! why the rest stays, and deletes what is ticked; `-PURGE` asks on the
//! command line (`purge_line.rs`). Both come here, and each is one undo step.

use od_commands::{Action, Cancelled, Editor, Options};
use od_core::purge::{blocked, purgeable, unnamed, Blocked, Category, Item, Purgeable};
use od_core::{BlockId, Document};

use crate::purge_delete::delete_actions;
use crate::text::plural;

/// Which page of the dialog shows.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Tab {
    #[default]
    Purgeable,
    NonPurgeable,
}

/// The Purge dialog while it is open.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    pub tab: Tab,
    /// What can go, and which of it is ticked.
    pub items: Vec<(Purgeable, bool)>,
    /// What stays, and why.
    pub blocked: Vec<Blocked>,
    /// The row of `blocked` that is open.
    pub detail: Option<usize>,
    /// The item picked in the tree, and its picture when it has one.
    pub selected: Option<Item>,
    pub preview: Option<(Item, eframe::egui::TextureHandle)>,
    /// How many zero-length objects and empty texts there are.
    pub zero_length: usize,
    pub empty_text: usize,
    /// A purge was started: list again once it is done.
    pub stale: bool,
}

impl crate::state::AppState {
    /// Open the Purge dialog on what can go now, nothing ticked.
    pub fn open_purge(&mut self) {
        self.purge.items.clear();
        self.refresh_purge();
        self.purge.open = true;
    }

    /// List what can go and what stays again, keeping the ticks of what is
    /// still there.
    pub fn refresh_purge(&mut self) {
        let doc = self.session().document();
        let (found, blocked) = (purgeable(doc), blocked(doc));
        let loose = unnamed(doc);
        let old = std::mem::take(&mut self.purge.items);
        self.purge.items = found
            .into_iter()
            .map(|p| {
                let ticked = old.iter().any(|(o, on)| o.item == p.item && *on);
                (p, ticked)
            })
            .collect();
        self.purge.blocked = blocked;
        self.purge.detail = None;
        self.purge.selected = None;
        self.purge.preview = None;
        self.purge.zero_length = loose.zero_length.len();
        self.purge.empty_text = loose.empty_text.len();
        self.purge.stale = false;
    }
}

/// A picture of what a block draws, or `None` when it is empty or too
/// detailed to draw.
pub fn block_picture(doc: &Document, id: BlockId) -> Option<od_canvas::preview::Preview> {
    let block = doc.block(id)?;
    let mut copy = doc.clone();
    let ids: Vec<_> = copy.objects().iter().map(|o| o.id).collect();
    copy.remove_many(&ids);
    for part in &block.entities {
        copy.add_with(part.entity.clone(), part.attrs.clone());
    }
    od_canvas::preview::render(&copy).ok()
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

/// What a purge may do: which named items (all when `None`), whether to
/// go on to what deleting them frees, whether to ask about each one first,
/// and which unnamed objects to delete too.
#[derive(Clone, Default)]
pub struct Purge {
    pub only: Option<Vec<Item>>,
    pub nested: bool,
    pub confirm: bool,
    pub zero_length: bool,
    pub empty_text: bool,
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

/// The first round: the named items asked for and the unnamed objects
/// switched on.
fn first_round(doc: &Document, purge: &Purge) -> Vec<Purgeable> {
    let mut found = purgeable(doc);
    if let Some(only) = &purge.only {
        found.retain(|p| only.contains(&p.item));
    }
    let loose = unnamed(doc);
    let count = |n: usize| format!("{n} object{}", if n == 1 { "" } else { "s" });
    if purge.zero_length && !loose.zero_length.is_empty() {
        let name = count(loose.zero_length.len());
        let item = Item::ZeroLength(loose.zero_length);
        found.push(Purgeable { item, name });
    }
    if purge.empty_text && !loose.empty_text.is_empty() {
        let name = count(loose.empty_text.len());
        let item = Item::EmptyText(loose.empty_text);
        found.push(Purgeable { item, name });
    }
    found
}

/// PURGE and -PURGE: delete what `purge` says. The questions come first and
/// change nothing; then everything is deleted as one undo step. With nested
/// items, each round is worked out on a copy of the drawing to find what
/// deleting it frees; every round deletes something, so it ends.
pub fn purge_run(ed: &mut Editor, purge: Purge) -> Result<(), Cancelled> {
    let mut sim = ed.read(|doc| doc.clone())?;
    if let Err(why) = sim.validate() {
        ed.say(format!(
            "PURGE refused: the drawing has an error ({why:?})."
        ));
        return Ok(());
    }
    let mut seen: Vec<Item> = purgeable(&sim).into_iter().map(|p| p.item).collect();
    let mut found = first_round(&sim, &purge);
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
        if !purge.nested {
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

#[cfg(test)]
#[path = "purge_cmd_tests.rs"]
mod tests;
