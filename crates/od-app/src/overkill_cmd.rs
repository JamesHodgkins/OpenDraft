//! OVERKILL: delete duplicate objects and merge lines and arcs that overlap
//! or meet end to end. The maths is `od_core::overkill`; this asks which
//! objects and which settings (the dialog, or `-OVERKILL` in
//! `overkill_line.rs`) and applies the outcome as one undo step.

use std::collections::HashSet;

use od_commands::{Cancelled, Editor, ModifyEntities, RemoveEntities};
use od_core::overkill::{overkill, Candidate, Ignore, Settings};
use od_core::{Attr, Attributes, Color, Document, Entity, EntityId, LayerId, LayoutId, Lineweight};

use crate::overkill_memory::{memory, remember};
use crate::text::plural;

/// What must match for two objects to be tidied against each other.
#[derive(PartialEq)]
struct Key {
    layout: Option<LayoutId>,
    layer: Option<LayerId>,
    color: Option<Attr<Color>>,
    linetype: Option<Attr<String>>,
    linetype_scale: Option<u64>,
    lineweight: Option<Attr<Lineweight>>,
}

fn key(layout: Option<LayoutId>, attrs: &Attributes, ignore: Ignore) -> Key {
    Key {
        layout,
        layer: (!ignore.layer).then_some(attrs.layer),
        color: (!ignore.color).then(|| attrs.color.clone()),
        linetype: (!ignore.linetype).then(|| attrs.linetype.clone()),
        linetype_scale: (!ignore.linetype_scale).then(|| attrs.linetype_scale.to_bits()),
        lineweight: (!ignore.lineweight).then(|| attrs.lineweight.clone()),
    }
}

/// The objects other objects are tied to: what an associative hatch was
/// made from, what a dimension's points sit on, what a leader annotates.
fn tied_to(doc: &Document) -> HashSet<EntityId> {
    let mut tied = HashSet::new();
    for object in doc.objects() {
        match &object.entity {
            Entity::Dimension(d) => tied.extend(d.assoc.iter().map(|a| a.target)),
            Entity::Hatch(h) if h.associative => tied.extend(h.source_ids()),
            Entity::Leader(l) => tied.extend(l.annotation),
            _ => {}
        }
    }
    tied
}

/// OVERKILL: select the objects (or use those already selected), then the
/// Object Comparison Settings dialog opens; OK tidies them. Objects only
/// compare against ones on the same layer, colour, linetype, linetype scale
/// and lineweight unless those are ignored.
pub fn overkill_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects")?;
    let settings = ed.ask_overkill(memory())?;
    remember(settings);
    run(ed, chosen, settings)
}

/// Tidy `chosen` with `settings`: the part both front ends share.
pub(crate) fn run(
    ed: &mut Editor,
    chosen: Vec<EntityId>,
    settings: Settings,
) -> Result<(), Cancelled> {
    let ignore = settings.ignore;
    let (found, skipped, protected) = ed.read(move |doc| {
        let tied = if settings.maintain_associative {
            tied_to(doc)
        } else {
            HashSet::new()
        };
        let mut found = Vec::new();
        let mut skipped = 0usize;
        for id in &chosen {
            let Some(o) = doc.object(*id) else { continue };
            if doc.is_selectable(o) {
                found.push((*id, o.entity.clone(), key(o.layout, &o.attrs, ignore)));
            } else {
                skipped += 1;
            }
        }
        let protected: Vec<bool> = found.iter().map(|f| tied.contains(&f.0)).collect();
        (found, skipped, protected)
    })?;
    if skipped > 0 {
        ed.say(format!(
            "{} on locked or hidden layers left alone.",
            plural(skipped, "object")
        ));
    }
    let mut keys: Vec<&Key> = Vec::new();
    let items: Vec<Candidate> = found
        .iter()
        .zip(&protected)
        .map(|((_, entity, k), protected)| {
            let group = keys.iter().position(|seen| *seen == k).unwrap_or_else(|| {
                keys.push(k);
                keys.len() - 1
            });
            Candidate {
                entity: entity.clone(),
                group,
                protected: *protected,
            }
        })
        .collect();
    let outcome = match overkill(&items, &settings) {
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
    let ids: Vec<EntityId> = found.iter().map(|f| f.0).collect();
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
    let tied = protected.iter().filter(|p| **p).count();
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
