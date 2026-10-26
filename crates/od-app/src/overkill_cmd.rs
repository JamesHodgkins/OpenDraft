//! OVERKILL: delete duplicate objects and merge lines and arcs that overlap
//! or meet end to end. The maths is `od_core::overkill`; this asks which
//! objects and which settings, and applies the outcome as one undo step.

use std::sync::Mutex;

use od_commands::{Cancelled, Editor};
use od_core::overkill::{overkill, Ignore, Settings};
use od_core::{Attr, Attributes, Color, Entity, EntityId, LayerId, LayoutId, Lineweight};

use crate::text::plural;

/// What the dialog was last closed with, offered again next time.
static REMEMBERED: Mutex<Option<Settings>> = Mutex::new(None);

fn remembered() -> Settings {
    REMEMBERED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or_default()
}

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

/// OVERKILL: select the objects (or use those already selected), then the
/// Object Comparison Settings dialog opens; OK tidies them. Objects only
/// compare against ones on the same layer, colour, linetype, linetype scale
/// and lineweight unless those are ignored.
pub fn overkill_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects")?;
    let settings = ed.ask_overkill(remembered())?;
    *REMEMBERED.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
    run(ed, chosen, settings)
}

fn run(ed: &mut Editor, chosen: Vec<EntityId>, settings: Settings) -> Result<(), Cancelled> {
    let ignore = settings.ignore;
    let found = ed.read(move |doc| -> Vec<(EntityId, Entity, Key)> {
        chosen
            .iter()
            .filter_map(|id| {
                let o = doc.object(*id)?;
                Some((*id, o.entity.clone(), key(o.layout, &o.attrs, ignore)))
            })
            .collect()
    })?;
    let mut keys: Vec<&Key> = Vec::new();
    let items: Vec<(Entity, usize)> = found
        .iter()
        .map(|(_, entity, k)| {
            let group = keys.iter().position(|seen| *seen == k).unwrap_or_else(|| {
                keys.push(k);
                keys.len() - 1
            });
            (entity.clone(), group)
        })
        .collect();
    let outcome = overkill(&items, &settings);
    if outcome.removed.is_empty() && outcome.changed.is_empty() {
        ed.say("No duplicate or overlapping objects found.");
        return Ok(());
    }
    let merged = outcome.changed.len();
    let deleted = outcome.removed.len();
    let ids: Vec<EntityId> = found.iter().map(|f| f.0).collect();
    ed.transaction("Overkill", move |tx| {
        for (i, entity) in outcome.changed {
            tx.modify(ids[i], move |_| entity);
        }
        let gone: Vec<EntityId> = outcome.removed.iter().map(|i| ids[*i]).collect();
        if !gone.is_empty() {
            tx.remove(gone);
        }
        Ok(())
    })?;
    ed.say(format!(
        "{} deleted, {} combined.",
        plural(deleted, "object"),
        plural(merged, "object")
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Point;

    fn line(x1: f64, x2: f64) -> Entity {
        Entity::Line {
            start: Point::new(x1, 0.0),
            end: Point::new(x2, 0.0),
        }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn killing(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.select_all();
        state.start_command("Overkill", overkill_objects);
        state.settle_command();
        let ask = state.waiting().expect("the dialog is asked for");
        assert_eq!(ask.wants, od_commands::Wants::OverkillDialog);
        state.reply(od_commands::Reply::Overkill(Settings::default()));
        state.settle_command();
        state
    }

    #[test]
    fn duplicates_go_and_overlaps_merge_in_one_undo_step() {
        let mut state = killing(&[
            line(0.0, 6.0),
            line(0.0, 6.0),
            line(4.0, 10.0),
            line(20.0, 21.0),
        ]);
        assert_eq!(entities(&state), vec![line(0.0, 10.0), line(20.0, 21.0)]);
        state.undo();
        assert_eq!(entities(&state).len(), 4);
    }

    #[test]
    fn nothing_to_tidy_changes_nothing() {
        let state = killing(&[line(0.0, 1.0), line(5.0, 6.0)]);
        assert_eq!(entities(&state).len(), 2);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("No duplicate or overlapping objects found.")));
    }
}
