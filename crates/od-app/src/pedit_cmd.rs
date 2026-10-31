//! PEDIT: edit a polyline (close or open it, join other objects on, set its
//! width, straighten its curves, reverse it, edit its vertices), or with
//! Multiple, the same to several at once. Lines, arcs and rectangles become
//! polylines first. The edits themselves are `od_core::pedit`.
//!
//! Not built: Fit, Spline, vertex Tangent and spline conversion.

use od_commands::{Answer, Cancelled, Editor, ModifyAttributes, NumberOptions, Options, Picked};
use od_core::pedit::{JoinType, PolyData};
use od_core::{Entity, EntityId};

use crate::defaults::EditorDefaults;
use crate::pedit_join::{ask_fuzz, join_group};
use crate::pedit_vertex::edit_vertices;

/// PEDIT: pick a polyline (or use the selection), then choose what to do
/// until Enter. Each change is one undo step.
pub fn pedit_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut chosen = ed.pickset().to_vec();
    loop {
        match chosen.len() {
            0 => {
                let options = Options::<Picked>::new("Select polyline")
                    .keyword("Multiple", "M")
                    .allow_empty();
                match ed.ask_object(options)? {
                    Answer::Value(Picked { id, .. }) => {
                        if data_of(ed, id)?.is_some() {
                            return edit_one(ed, id);
                        }
                        refuse(ed, id)?;
                    }
                    Answer::Keyword(_) => chosen = ed.get_selection("Select objects")?,
                    Answer::Enter => return Ok(()),
                }
            }
            1 => {
                if data_of(ed, chosen[0])?.is_some() {
                    return edit_one(ed, chosen[0]);
                }
                refuse(ed, chosen[0])?;
                chosen.clear();
            }
            _ => return edit_many(ed, chosen),
        }
    }
}

/// Say why `id` cannot be edited.
fn refuse(ed: &Editor, id: EntityId) -> Result<(), Cancelled> {
    let spline = ed.read(move |doc| matches!(doc.get(id), Some(Entity::Spline(_))))?;
    ed.say(if spline {
        "Spline-to-polyline conversion is not yet supported; object was not changed."
    } else {
        "Object is not a polyline."
    });
    Ok(())
}

/// The polyline data of `id`, if it is something PEDIT can edit.
pub(crate) fn data_of(ed: &Editor, id: EntityId) -> Result<Option<PolyData>, Cancelled> {
    ed.read(move |doc| doc.get(id).and_then(PolyData::of))
}

/// Replace the object `id` with `data`, as one undo step.
pub(crate) fn store(ed: &Editor, id: EntityId, data: PolyData) -> Result<(), Cancelled> {
    let entity = data.into_entity();
    ed.transaction("Edit Polyline", move |tx| {
        tx.modify(id, move |_| entity);
        Ok(())
    })
}

fn is_polyline(ed: &Editor, id: EntityId) -> Result<bool, Cancelled> {
    ed.read(move |doc| matches!(doc.get(id), Some(Entity::Polyline { .. })))
}

/// Ask for one of `options`' keywords, typed or clicked; `None` for Enter.
pub(crate) fn ask_option(
    ed: &Editor,
    options: Options<String>,
) -> Result<Option<String>, Cancelled> {
    ed.ask_keyword(options)
}

fn edit_one(ed: &mut Editor, id: EntityId) -> Result<(), Cancelled> {
    if data_of(ed, id)?.is_none() {
        return refuse(ed, id);
    }
    let mut steps = 0usize;
    if !is_polyline(ed, id)? {
        // PEDITACCEPT 1 turns it into one without asking.
        let turn = if ed.defaults().pedit.get().accept {
            "Yes".to_owned()
        } else {
            ed.get_keyword(
                Options::new("Object selected is not a polyline. Do you want to turn it into one?")
                    .keyword("Yes", "Y")
                    .keyword("No", "N")
                    .default("Yes".to_owned()),
            )?
        };
        if turn == "No" {
            return Ok(());
        }
        if let Some(data) = data_of(ed, id)? {
            store(ed, id, data)?;
            steps += 1;
        }
    }
    loop {
        let Some(data) = data_of(ed, id)? else {
            return Ok(());
        };
        let toggle = if data.closed { "Open" } else { "Close" };
        let options = Options::<String>::new("Enter an option")
            .keyword(toggle, &toggle[..1])
            .keyword("Join", "J")
            .keyword("Width", "W")
            .keyword("Edit vertex", "E")
            .keyword("Decurve", "D")
            .keyword("Ltype gen", "L")
            .keyword("Reverse", "R")
            .keyword("Undo", "U");
        let Some(key) = ask_option(ed, options)? else {
            return Ok(());
        };
        match key.as_str() {
            "Ltype gen" => {
                if set_generation(ed, &[id])? {
                    steps += 1;
                }
            }
            "Close" | "Open" => {
                store(ed, id, data.with_closed(!data.closed))?;
                steps += 1;
            }
            "Join" => {
                if data.closed {
                    ed.say("A closed polyline cannot be joined to.");
                    continue;
                }
                let others =
                    ed.ask_selection(Options::new("Select objects to join").allow_empty())?;
                if join_group(ed, vec![id], others, 0.0, JoinType::Add)? > 0 {
                    steps += 1;
                }
            }
            "Width" => {
                if let Some(w) = ask_width(ed)? {
                    store(ed, id, data.with_width(w))?;
                    steps += 1;
                }
            }
            "Edit vertex" => steps += edit_vertices(ed, id)?,
            "Decurve" => {
                store(ed, id, data.decurved())?;
                steps += 1;
            }
            "Reverse" => {
                store(ed, id, data.reversed())?;
                steps += 1;
            }
            _ => {
                if steps == 0 {
                    ed.say("Nothing to undo.");
                } else {
                    ed.undo()?;
                    steps -= 1;
                }
            }
        }
    }
}

/// The width to give every segment; `None` if it was refused.
fn ask_width(ed: &Editor) -> Result<Option<f64>, Cancelled> {
    let w = ed.get_distance(NumberOptions::new("Specify new width for all segments"))?;
    if w < 0.0 {
        ed.say("Requires a value of zero or more.");
        return Ok(None);
    }
    Ok(Some(w))
}

/// Ltype gen: ask whether the linetype pattern runs on through the vertices
/// and set that for `ids`. `false` when nothing changed.
fn set_generation(ed: &Editor, ids: &[EntityId]) -> Result<bool, Cancelled> {
    let ids = ids.to_vec();
    let all = ids.clone();
    let attrs = ed.read(move |doc| {
        all.iter()
            .filter_map(|id| doc.object(*id).map(|o| (*id, o.attrs.clone())))
            .collect::<Vec<_>>()
    })?;
    let on = attrs.first().is_some_and(|(_, a)| a.continuous_generation);
    let answer = ed.get_keyword(
        Options::new("Enter polyline linetype generation option")
            .keyword("ON", "ON")
            .keyword("OFF", "OFF")
            .default(if on { "ON" } else { "OFF" }.to_owned()),
    )?;
    let on = answer == "ON";
    let changes: Vec<_> = attrs
        .into_iter()
        .filter(|(_, a)| a.continuous_generation != on)
        .map(|(id, mut a)| {
            a.continuous_generation = on;
            (id, a)
        })
        .collect();
    if changes.is_empty() {
        return Ok(false);
    }
    ed.transaction("Edit Polyline", move |tx| {
        tx.perform(ModifyAttributes::new("Edit Polyline", changes));
        Ok(())
    })?;
    Ok(true)
}

fn edit_many(ed: &mut Editor, chosen: Vec<EntityId>) -> Result<(), Cancelled> {
    let kinds = {
        let ids = chosen.clone();
        ed.read(move |doc| -> Vec<(EntityId, Entity)> {
            ids.iter()
                .filter_map(|id| doc.get(*id).map(|e| (*id, e.clone())))
                .collect()
        })?
    };
    let mut ids: Vec<EntityId> = kinds
        .iter()
        .filter(|(_, e)| PolyData::of(e).is_some())
        .map(|(id, _)| *id)
        .collect();
    let others = kinds
        .iter()
        .filter(|(_, e)| PolyData::of(e).is_some() && !matches!(e, Entity::Polyline { .. }))
        .count();
    if ids.is_empty() {
        ed.say("No polylines or objects that can become polylines were selected.");
        return Ok(());
    }
    if others > 0 && !ed.defaults().pedit.get().accept {
        let turn = ed.get_keyword(
            Options::new("Convert lines, arcs and rectangles to polylines?")
                .keyword("Yes", "Y")
                .keyword("No", "N")
                .default("Yes".to_owned()),
        )?;
        if turn == "No" {
            ids.retain(|id| {
                kinds
                    .iter()
                    .any(|(k, e)| k == id && matches!(e, Entity::Polyline { .. }))
            });
        }
    }
    if ids.is_empty() {
        return Ok(());
    }
    let mut steps = 0usize;
    loop {
        let options = Options::<String>::new("Enter an option")
            .keyword("Close", "C")
            .keyword("Open", "O")
            .keyword("Join", "J")
            .keyword("Width", "W")
            .keyword("Decurve", "D")
            .keyword("Ltype gen", "L")
            .keyword("Reverse", "R")
            .keyword("Undo", "U");
        let Some(key) = ask_option(ed, options)? else {
            return Ok(());
        };
        let done = match key.as_str() {
            "Ltype gen" => set_generation(ed, &ids)?,
            "Close" => change_all(ed, &ids, |d| d.with_closed(true))?,
            "Open" => change_all(ed, &ids, |d| d.with_closed(false))?,
            "Decurve" => change_all(ed, &ids, PolyData::decurved)?,
            "Reverse" => change_all(ed, &ids, PolyData::reversed)?,
            "Width" => match ask_width(ed)? {
                Some(w) => {
                    let widen = move |d: &PolyData| d.with_width(w);
                    change_all(ed, &ids, widen)?
                }
                None => false,
            },
            "Join" => {
                let (fuzz, kind) = ask_fuzz(ed)?;
                join_group(ed, ids.clone(), Vec::new(), fuzz, kind)? > 0
            }
            _ => {
                if steps == 0 {
                    ed.say("Nothing to undo.");
                } else {
                    ed.undo()?;
                    steps -= 1;
                }
                false
            }
        };
        if done {
            steps += 1;
            // Joining removes objects; keep only those still there.
            let all = ids.clone();
            ids =
                ed.read(move |doc| all.into_iter().filter(|i| doc.get(*i).is_some()).collect())?;
        }
    }
}

/// Apply `change` to each object that can be a polyline, as one undo step.
fn change_all(
    ed: &Editor,
    ids: &[EntityId],
    change: impl Fn(&PolyData) -> PolyData + Send + 'static,
) -> Result<bool, Cancelled> {
    let ids = ids.to_vec();
    let edits: Vec<(EntityId, Entity)> = ed.read(move |doc| {
        ids.iter()
            .filter_map(|id| {
                let data = PolyData::of(doc.get(*id)?)?;
                Some((*id, change(&data).into_entity()))
            })
            .collect()
    })?;
    if edits.is_empty() {
        return Ok(false);
    }
    ed.transaction("Edit Polyline", move |tx| {
        for (id, entity) in edits {
            tx.modify(id, move |_| entity);
        }
        Ok(())
    })?;
    Ok(true)
}

#[cfg(test)]
#[path = "pedit_join_tests.rs"]
mod join_tests;
#[cfg(test)]
#[path = "pedit_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "pedit_vertex_tests.rs"]
mod vertex_tests;
