//! OVERKILL: delete duplicate objects and merge lines and arcs that overlap
//! or meet end to end. The maths is `od_core::overkill`; this asks which
//! objects and which settings (the dialog, or `-OVERKILL` in
//! `overkill_line.rs`) and applies the outcome as one undo step.

use od_commands::{Cancelled, Editor, ModifyEntities, RemoveEntities};
use od_core::overkill::{overkill, prepare, Settings};
use od_core::EntityId;

use crate::defaults::EditorDefaults;
use crate::text::plural;

/// OVERKILL: select the objects (or use those already selected), then the
/// Object Comparison Settings dialog opens; OK tidies them. Objects only
/// compare against ones on the same layer, colour, linetype, linetype scale
/// and lineweight unless those are ignored.
pub fn overkill_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects")?;
    let settings = ed.ask_overkill(ed.defaults().overkill.get())?;
    ed.defaults().overkill.change(|m| *m = settings);
    run(ed, chosen, settings)
}

/// Tidy `chosen` with `settings`: the part both front ends share.
pub(crate) fn run(
    ed: &mut Editor,
    chosen: Vec<EntityId>,
    settings: Settings,
) -> Result<(), Cancelled> {
    let prepared = ed.read(move |doc| prepare(doc, &chosen, &settings, false))?;
    if prepared.skipped > 0 {
        ed.say(format!(
            "{} on locked or hidden layers left alone.",
            plural(prepared.skipped, "object")
        ));
    }
    let outcome = match overkill(&prepared.items, &settings) {
        Ok(outcome) => outcome,
        Err(why) => {
            ed.say(format!("OVERKILL did nothing: {why}."));
            return Ok(());
        }
    };
    if outcome.removed.is_empty() && outcome.changed.is_empty() {
        ed.say("No duplicate or overlapping objects found.");
        return Ok(());
    }
    let merged = outcome.changed.len();
    let deleted = outcome.removed.len();
    let tied = prepared.protected();
    let ids = prepared.ids;
    ed.transaction("Overkill", move |tx| {
        let changes = outcome
            .changed
            .into_iter()
            .map(|(i, entity)| (ids[i], entity))
            .collect::<Vec<_>>();
        if !changes.is_empty() {
            tx.perform(ModifyEntities::new("Overkill", changes));
        }
        let gone: Vec<EntityId> = outcome.removed.iter().map(|i| ids[*i]).collect();
        if !gone.is_empty() {
            tx.perform(RemoveEntities::new(gone));
        }
        Ok(())
    })?;
    ed.say(format!(
        "{} deleted, {} combined.",
        plural(deleted, "object"),
        plural(merged, "object")
    ));
    if tied > 0 {
        ed.say(format!(
            "{} that other objects are tied to were not deleted or changed.",
            plural(tied, "object")
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "overkill_cmd_tests.rs"]
mod tests;
