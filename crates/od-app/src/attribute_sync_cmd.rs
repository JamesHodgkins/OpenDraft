//! ATTSYNC: bring every reference of a block up to date with the block's
//! attribute definitions. Values stay; tags, modes, text settings and
//! places come from the definitions again; new definitions add an attribute
//! with its default; references whose definition is gone are kept at the end
//! or removed, as the user says.

use od_commands::{Answer, Cancelled, Editor, ModifyEntities, Options};
use od_core::block::sync_references;
use od_core::{BlockId, Entity, EntityId};

use crate::attribute_edit::pick_attributed_insert;
use crate::text::plural;

/// What synchronising a block changes.
#[derive(Default)]
struct Plan {
    inserts: usize,
    matched: usize,
    added: usize,
    orphaned: usize,
    edits: Vec<(EntityId, Entity)>,
}

/// The change for every insert of `block` in the drawing's spaces.
fn plan(doc: &od_core::Document, block: BlockId, remove_orphans: bool) -> Plan {
    let mut plan = Plan::default();
    for object in doc.objects() {
        let Entity::Insert(insert) = &object.entity else {
            continue;
        };
        if insert.block.id != block {
            continue;
        }
        let synced = sync_references(&insert.block, &insert.attributes, remove_orphans);
        plan.inserts += 1;
        plan.matched += synced.matched;
        plan.added += synced.added;
        plan.orphaned += synced.orphaned;
        if synced.references != insert.attributes {
            let mut changed = (**insert).clone();
            changed.attributes = synced.references;
            plan.edits
                .push((object.id, Entity::Insert(Box::new(changed))));
        }
    }
    plan
}

fn ask_yes_no(ed: &Editor, question: String, default: &str) -> Result<bool, Cancelled> {
    let options = Options::<String>::new(question)
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default(default.to_owned());
    Ok(ed.get_keyword(options)? == "Yes")
}

/// The block to synchronise: that of the preselected reference, one named,
/// or that of a reference picked.
fn choose_block(ed: &mut Editor) -> Result<Option<BlockId>, Cancelled> {
    let of_insert = |ed: &Editor, id| {
        ed.read(move |doc| match doc.get(id) {
            Some(Entity::Insert(i)) => Some(i.block.id),
            _ => None,
        })
    };
    if let [id] = ed.pickset() {
        let id = *id;
        if let Some(block) = of_insert(ed, id)? {
            return Ok(Some(block));
        }
    }
    loop {
        let options = Options::<String>::new("Enter an option [?/Name/Select]")
            .keyword("?", "?")
            .keyword("Name", "N")
            .keyword("Select", "S")
            .default("Select".to_owned());
        match ed.get_keyword(options)?.as_str() {
            "?" => {
                let names = ed.read(|doc| {
                    doc.blocks()
                        .iter()
                        .filter(|b| b.has_attributes())
                        .map(|b| b.name.clone())
                        .collect::<Vec<_>>()
                })?;
                if names.is_empty() {
                    ed.say("No blocks with attributes.");
                }
                for name in names {
                    ed.say(name);
                }
            }
            "Name" => {
                let Answer::Value(name) =
                    ed.ask_string(Options::<String>::new("Enter block name"))?
                else {
                    return Ok(None);
                };
                let found = ed.read(move |doc| doc.block_named(&name).map(|b| b.id))?;
                match found {
                    Some(id) => return Ok(Some(id)),
                    None => ed.say("Block not found."),
                }
            }
            _ => {
                let id = pick_attributed_insert(ed, true)?;
                if let Some(block) = of_insert(ed, id)? {
                    return Ok(Some(block));
                }
            }
        }
    }
}

pub fn sync_attributes(ed: &mut Editor) -> Result<(), Cancelled> {
    let Some(block) = choose_block(ed)? else {
        return Ok(());
    };
    let kept = ed.read(move |doc| plan(doc, block, false))?;
    if kept.inserts == 0 {
        ed.say("No block references to update.");
        return Ok(());
    }
    let remove = kept.orphaned > 0
        && ask_yes_no(
            ed,
            format!(
                "{} no longer in the block definition. Remove them?",
                plural(kept.orphaned, "attribute")
            ),
            "No",
        )?;
    let plan = if remove {
        ed.read(move |doc| plan(doc, block, true))?
    } else {
        kept
    };
    if plan.edits.is_empty() {
        ed.say(format!(
            "{} already up to date.",
            plural(plan.inserts, "block reference")
        ));
        return Ok(());
    }
    ed.say(format!(
        "{}: {} kept, {} added, {} {}.",
        plural(plan.inserts, "block reference"),
        plural(plan.matched, "attribute"),
        plan.added,
        plan.orphaned,
        if remove { "removed" } else { "kept as extra" },
    ));
    if !ask_yes_no(
        ed,
        "Settings changed on a reference are lost. Update?".to_owned(),
        "Yes",
    )? {
        return Ok(());
    }
    let edits = plan.edits;
    ed.transaction("Synchronize attributes", move |tx| {
        tx.perform(ModifyEntities::new("Synchronize attributes", edits));
        Ok(())
    })
}

#[cfg(test)]
mod tests {

    use od_commands::SetBlocks;

    use super::*;
    use crate::attribute_test_util::{attributed_insert, definition, values};

    /// Block "Label" changed to attributes B, C: A is gone, C is new. The
    /// table and the inserts change together, as BEDIT's save does.
    fn redefine_label(ed: &mut Editor) -> Result<(), Cancelled> {
        ed.transaction("Redefine", |tx| {
            let doc = tx.document();
            let mut block = (**doc.block_named("Label").unwrap()).clone();
            block
                .entities
                .retain(|e| !matches!(&e.entity, Entity::AttributeDefinition(d) if d.tag == "A"));
            block.entities.push(definition(3, "C", 0));
            let change = doc.redefined_blocks(block).unwrap();
            tx.perform(SetBlocks::new("Block", change.blocks));
            tx.perform(ModifyEntities::new("Redefine", change.objects));
            Ok(())
        })
    }

    fn redefine(state: &mut crate::state::AppState) {
        state.start_command("REDEFINE", redefine_label);
        state.settle_command();
    }

    #[test]
    fn a_redefinition_leaves_references_alone_until_the_sync() {
        let (mut state, id) = attributed_insert();
        state.session().document().validate().unwrap();
        // A user's own value on B.
        let Some(Entity::Insert(i)) = state.session().document().get(id) else {
            panic!()
        };
        let mut edited = (**i).clone();
        edited.attributes[1].value = "mine".into();
        state.perform(ModifyEntities::new(
            "Edit",
            vec![(id, Entity::Insert(Box::new(edited)))],
        ));
        redefine(&mut state);
        assert_eq!(values(&state, id), ["dflt", "mine"], "A is now an orphan");
        assert_eq!(state.session().document().validate(), Ok(()));

        state.select_ids([id], false);
        state.start_command("ATTSYNC", sync_attributes);
        state.settle_command();
        // Keep the orphan? No: remove it. Then confirm.
        for answer in ["Y", ""] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        assert!(!state.command_running());
        assert_eq!(values(&state, id), ["mine", "dflt"], "B kept, C added");
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "mine"]);
    }

    #[test]
    fn an_up_to_date_block_changes_nothing() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let before = state.session().revision();
        state.start_command("ATTSYNC", sync_attributes);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(state.session().revision(), before);
        assert!(state.cmdline.log.iter().any(|l| l.contains("up to date")));
    }
}
