//! BLOCK: make a block definition from objects. The block table's rules (names, redefinition, nesting) are
//! in `od_core::block`; INSERT is in `insert_cmd.rs`.

use od_commands::{Answer, Cancelled, Editor, ModifyEntities, Options, RemoveEntities, SetBlocks};
use od_core::block::block_name_problem;
use od_core::{BlockDef, BlockId, Document, Entity, Insert};

use crate::text::plural;

/// The block names with how often each is inserted, for the `?` option.
pub(crate) fn list_blocks(doc: &Document) -> Vec<String> {
    if doc.blocks().is_empty() {
        return vec!["No blocks are defined in this drawing.".to_owned()];
    }
    let mut lines = vec!["Defined blocks:".to_owned()];
    for block in doc.blocks() {
        let uses = doc.block_references(block.id).len();
        lines.push(format!(
            "  \"{}\": {}",
            block.name,
            plural(uses, "reference")
        ));
    }
    lines
}

/// The answer to a block-name prompt: a name, a request for the list, or
/// nothing (Enter).
pub(crate) enum NameAnswer {
    Name(String),
    List,
    Done,
}

/// Ask for a block name. Options already on `options` (block names to
/// pick from) answer with their label; `?` is added last.
pub(crate) fn ask_block_name(
    ed: &Editor,
    options: Options<String>,
) -> Result<NameAnswer, Cancelled> {
    Ok(match ed.ask_string(options.keyword("?", "?"))? {
        Answer::Keyword(label) if label != "?" => NameAnswer::Name(label),
        Answer::Keyword(_) => NameAnswer::List,
        Answer::Value(text) if text.trim() == "?" => NameAnswer::List,
        Answer::Value(text) if !text.trim().is_empty() => NameAnswer::Name(text.trim().to_owned()),
        Answer::Value(_) | Answer::Enter => NameAnswer::Done,
    })
}

/// BLOCK, worded as AutoCAD's -BLOCK: a name, a base point, then the
/// objects (those already selected, if any). The objects become one
/// reference to the new block at the base point, as the BLOCK dialog's
/// default "Convert to block" does. A name already in use asks before
/// redefining it, which updates every reference. One undo step.
pub fn define_block(ed: &mut Editor) -> Result<(), Cancelled> {
    let (name, existing) = loop {
        let name = match ask_block_name(ed, Options::new("Enter block name"))? {
            NameAnswer::Done => return Ok(()),
            NameAnswer::List => {
                for line in ed.read(list_blocks)? {
                    ed.say(line);
                }
                continue;
            }
            NameAnswer::Name(name) => name,
        };
        if let Some(why) = block_name_problem(&name) {
            ed.say(why);
            continue;
        }
        let lookup = name.clone();
        let existing =
            ed.read(move |doc| doc.block_named(&lookup).map(|b| (b.id, b.name.clone())))?;
        let Some((id, shown)) = existing else {
            break (name, None);
        };
        let question = Options::new(format!("Block \"{shown}\" already exists. Redefine it?"))
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("No".to_owned());
        if ed.get_keyword(question)? == "Yes" {
            break (shown, Some(id));
        }
    };
    let base = ed.get_point("Specify insertion base point")?;
    let chosen = ed.pickset_or_select("Select objects")?;
    if chosen.is_empty() {
        ed.say("No objects selected; the block was not made.");
        return Ok(());
    }
    let redefining = existing.is_some();
    let label = name.clone();
    let made = ed.transaction("Block", move |tx| {
        let doc = tx.document();
        let id = existing.unwrap_or(BlockId(doc.next_block_id()));
        let objects = doc.objects().iter().filter(|o| chosen.contains(&o.id));
        let def = BlockDef::from_objects(id, &name, base, objects);
        let count = def.entities.len();
        let change = match doc.redefined_blocks(def) {
            Ok(change) => change,
            Err(why) => return Ok(Err(why.to_owned())),
        };
        let Some(record) = change.blocks.iter().find(|b| b.id == id).cloned() else {
            return Ok(Err(
                "The block could not be added to the block table.".to_owned()
            ));
        };
        tx.perform(SetBlocks::new("Block", change.blocks));
        if !change.objects.is_empty() {
            tx.perform(ModifyEntities::new("Redefine block", change.objects));
        }
        tx.perform(RemoveEntities::new(chosen));
        tx.add(Entity::Insert(Box::new(Insert::new(record, base))));
        Ok(Ok(count))
    })?;
    match made {
        Ok(count) => {
            let done = if redefining { "redefined" } else { "defined" };
            ed.say(format!(
                "Block \"{label}\" {done} with {}.",
                plural(count, "object")
            ));
        }
        Err(why) => ed.say(why),
    }
    Ok(())
}

#[cfg(test)]
#[path = "block_cmd_tests.rs"]
mod tests;
