//! OVERKILL: delete duplicate objects and merge lines and arcs that overlap
//! or meet end to end. The maths is `od_core::overkill`; this asks which
//! objects and which settings, and applies the outcome as one undo step.

use od_commands::{Cancelled, Editor, Options};
use od_core::overkill::{overkill, Settings};
use od_core::units::format_length;
use od_core::{Attr, Attributes, Color, Entity, EntityId, LayerId, LayoutId, Lineweight, Units};

use crate::text::plural;

/// Which properties may differ between two objects that are still compared.
#[derive(Clone, Copy, Default)]
struct Ignore {
    layer: bool,
    color: bool,
    linetype: bool,
    lineweight: bool,
}

/// What must match for two objects to be tidied against each other.
#[derive(PartialEq)]
struct Key {
    layout: Option<LayoutId>,
    layer: Option<LayerId>,
    color: Option<Attr<Color>>,
    linetype: Option<Attr<String>>,
    lineweight: Option<Attr<Lineweight>>,
}

fn key(layout: Option<LayoutId>, attrs: &Attributes, ignore: Ignore) -> Key {
    Key {
        layout,
        layer: (!ignore.layer).then_some(attrs.layer),
        color: (!ignore.color).then(|| attrs.color.clone()),
        linetype: (!ignore.linetype).then(|| attrs.linetype.clone()),
        lineweight: (!ignore.lineweight).then(|| attrs.lineweight.clone()),
    }
}

fn yes_no(on: bool) -> &'static str {
    if on {
        "Yes"
    } else {
        "No"
    }
}

fn say_settings(ed: &Editor, units: &Units, settings: &Settings, ignore: Ignore) {
    let names = [
        (ignore.layer, "Layer"),
        (ignore.color, "Color"),
        (ignore.linetype, "Linetype"),
        (ignore.lineweight, "Lineweight"),
    ];
    let ignored: Vec<&str> = names.iter().filter(|n| n.0).map(|n| n.1).collect();
    ed.say(format!(
        "Tolerance = {}, Ignore = {}, Combine partly overlapping = {}, Combine end to end = {}",
        format_length(units, settings.tolerance),
        if ignored.is_empty() {
            "None".to_owned()
        } else {
            ignored.join(", ")
        },
        yes_no(settings.overlapping),
        yes_no(settings.end_to_end)
    ));
}

/// Toggle which properties are ignored, until Enter.
fn ask_ignore(ed: &Editor, ignore: &mut Ignore) -> Result<(), Cancelled> {
    loop {
        let options = Options::<String>::new(
            "Property to ignore, Enter when done [Layer/Color/Linetype/liNeweight]",
        )
        .keyword("Layer", "L")
        .keyword("Color", "C")
        .keyword("Linetype", "T")
        .keyword("Lineweight", "W");
        match ed.ask_keyword(options)?.as_deref() {
            Some("Layer") => ignore.layer = !ignore.layer,
            Some("Color") => ignore.color = !ignore.color,
            Some("Linetype") => ignore.linetype = !ignore.linetype,
            Some("Lineweight") => ignore.lineweight = !ignore.lineweight,
            _ => return Ok(()),
        }
    }
}

/// OVERKILL: select the objects (or use those already selected), adjust the
/// settings if needed and press Enter. Objects only compare against ones on
/// the same layer, colour, linetype and lineweight unless those are ignored.
pub fn overkill_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects")?;
    let units = ed.read(|doc| *doc.units())?;
    let mut settings = Settings::default();
    let mut ignore = Ignore::default();
    loop {
        say_settings(ed, &units, &settings, ignore);
        let options = Options::<String>::new(
            "Enter an option [Tolerance/Ignore/Overlapping/End to end], Enter to run",
        )
        .keyword("Tolerance", "T")
        .keyword("Ignore", "I")
        .keyword("Overlapping", "O")
        .keyword("End to end", "E");
        match ed.ask_keyword(options)?.as_deref() {
            Some("Tolerance") => {
                let ask = Options::<f64>::new("Specify tolerance").default(settings.tolerance);
                let value = ed.get_distance(ask)?;
                if value >= 0.0 {
                    settings.tolerance = value;
                } else {
                    ed.say("The tolerance cannot be negative.");
                }
            }
            Some("Ignore") => ask_ignore(ed, &mut ignore)?,
            Some("Overlapping") => settings.overlapping = !settings.overlapping,
            Some("End to end") => settings.end_to_end = !settings.end_to_end,
            _ => break,
        }
    }
    run(ed, chosen, settings, ignore)
}

fn run(
    ed: &mut Editor,
    chosen: Vec<EntityId>,
    settings: Settings,
    ignore: Ignore,
) -> Result<(), Cancelled> {
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
        state.answer_enter();
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
