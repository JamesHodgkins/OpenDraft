//! PEDIT's Edit vertex: a marker steps along the polyline's vertices and
//! the options insert, move, straighten, break or widen at it. The edits are
//! `od_core::pedit`. Not built: Tangent.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::pedit::PolyData;
use od_core::{Entity, EntityId, Point};

use crate::pedit_cmd::{data_of, store};

/// Half the size of a vertex cross, in screen points.
const ARM: f64 = 6.0;

/// A cross at each of `at`, `ARM` screen points to an arm: `unit` is how
/// many drawing units a screen point spans.
fn markers(points: &[Point], at: &[usize], unit: f64) -> Vec<Entity> {
    let size = ARM * unit;
    at.iter()
        .filter_map(|&i| points.get(i))
        .flat_map(|p| {
            let arm = |dx: f64, dy: f64| Entity::Line {
                start: Point::new(p.x - dx, p.y - dy),
                end: Point::new(p.x + dx, p.y + dy),
            };
            [arm(size, size), arm(size, -size)]
        })
        .collect()
}

/// Whether a segment starts at vertex `i`.
fn has_segment(data: &PolyData, i: usize) -> bool {
    data.closed || i + 1 < data.points.len()
}

/// Edit vertices of `id` until eXit; returns how many undo steps were made.
pub fn edit_vertices(ed: &mut Editor, id: EntityId) -> Result<usize, Cancelled> {
    let (mut v, mut steps) = (0usize, 0usize);
    loop {
        let Some(data) = data_of(ed, id)? else {
            return Ok(steps);
        };
        let n = data.points.len();
        v = v.min(n - 1);
        let points = data.points.clone();
        let options = PointOptions::new("Enter a vertex editing option (Enter for Next)")
            .keyword("Next", "N")
            .keyword("Previous", "P")
            .keyword("Break", "B")
            .keyword("Insert", "I")
            .keyword("Move", "M")
            .keyword("Regen", "R")
            .keyword("Straighten", "S")
            .keyword("Width", "W")
            .keyword("eXit", "X")
            .marker(move |unit| markers(&points, &[v], unit))
            .allow_empty();
        let key = match ed.ask_point(options)? {
            Answer::Keyword(key) => key,
            Answer::Enter => "Next".to_owned(),
            Answer::Value(_) => {
                ed.say("Invalid option keyword.");
                continue;
            }
        };
        match key.as_str() {
            "Next" => v = (v + 1).min(n - 1),
            "Previous" => v = v.saturating_sub(1),
            "Regen" => {}
            "eXit" => return Ok(steps),
            "Insert" => {
                if !has_segment(&data, v) {
                    ed.say("There is no segment after the last vertex to insert into.");
                    continue;
                }
                let options =
                    PointOptions::new("Specify location for new vertex").base(data.points[v]);
                let at = ed.get_point(options)?;
                match data.with_inserted(v, at) {
                    Some(inserted) => {
                        store(ed, id, inserted)?;
                        v += 1;
                        steps += 1;
                    }
                    None => ed.say("That point is at the end of the arc segment."),
                }
            }
            "Move" => {
                let options = PointOptions::new("Specify new location for marked vertex")
                    .base(data.points[v]);
                let at = ed.get_point(options)?;
                store(ed, id, data.with_moved(v, at))?;
                steps += 1;
            }
            "Width" => {
                if !has_segment(&data, v) {
                    ed.say("There is no segment after the last vertex to widen.");
                    continue;
                }
                let (start, _) = od_core::width::at(&data.widths, v);
                let first =
                    NumberOptions::new("Specify starting width for next segment").default(start);
                let w0 = ed.get_distance(first)?;
                let last = NumberOptions::new("Specify ending width for next segment").default(w0);
                let w1 = ed.get_distance(last)?;
                if w0 < 0.0 || w1 < 0.0 {
                    ed.say("Requires a value of zero or more.");
                    continue;
                }
                store(ed, id, data.with_segment_width(v, w0, w1))?;
                steps += 1;
            }
            "Straighten" => {
                if let Some(w) = other_vertex(ed, &data, v)? {
                    let (i, j) = (v.min(w), v.max(w));
                    match data.straightened(i, j) {
                        Some(straight) => {
                            store(ed, id, straight)?;
                            steps += 1;
                            v = i;
                        }
                        None => ed.say("Pick a second vertex to straighten to."),
                    }
                }
            }
            _ => {
                if let Some(w) = other_vertex(ed, &data, v)? {
                    return Ok(steps + usize::from(break_at(ed, id, &data, v, w)?));
                }
            }
        }
    }
}

/// Step a second marker from vertex `from` until Go (the vertex) or eXit.
fn other_vertex(ed: &Editor, data: &PolyData, from: usize) -> Result<Option<usize>, Cancelled> {
    let n = data.points.len();
    let mut w = from;
    loop {
        let points = data.points.clone();
        let options = PointOptions::new("Enter an option (Enter for Next)")
            .keyword("Next", "N")
            .keyword("Previous", "P")
            .keyword("Go", "G")
            .keyword("eXit", "X")
            .marker(move |unit| markers(&points, &[from, w], unit))
            .allow_empty();
        match ed.ask_point(options)? {
            Answer::Keyword(key) => match key.as_str() {
                "Next" => w = (w + 1).min(n - 1),
                "Previous" => w = w.saturating_sub(1),
                "Go" => return Ok(Some(w)),
                _ => return Ok(None),
            },
            Answer::Enter => w = (w + 1).min(n - 1),
            Answer::Value(_) => ed.say("Invalid option keyword."),
        }
    }
}

/// Cut out the stretch between vertices `a` and `b`: the first piece stays
/// the object, any other is a new one on the same layer. `false` when
/// nothing would change.
fn break_at(
    ed: &Editor,
    id: EntityId,
    data: &PolyData,
    a: usize,
    b: usize,
) -> Result<bool, Cancelled> {
    let Some(mut pieces) = data.broken(a, b) else {
        ed.say("Nothing to break there.");
        return Ok(false);
    };
    let attrs = ed.read(move |doc| doc.object(id).map(|o| o.attrs.clone()))?;
    let Some(attrs) = attrs else {
        return Ok(false);
    };
    let first = pieces.remove(0).into_entity();
    let rest: Vec<Entity> = pieces.into_iter().map(PolyData::into_entity).collect();
    ed.transaction("Edit Polyline", move |tx| {
        tx.modify(id, move |_| first);
        if !rest.is_empty() {
            tx.add_with(rest.into_iter().map(|e| (e, attrs.clone())).collect());
        }
        Ok(())
    })?;
    Ok(true)
}
