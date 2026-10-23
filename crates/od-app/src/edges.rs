//! TRIM and EXTEND: change objects against the edges other objects make. Both
//! ask for edges, then for objects to change, and Shift-click does the other
//! one's job. Written against the `Editor` like the other commands; the
//! geometry is `od_core`'s `intersections`, `split_at` and `extended_to`.

use od_commands::{Answer, Cancelled, Editor, Options, Picked};
use od_core::{End, Entity, EntityId, Point};

use crate::text::plural;

/// What is left of `target` when the piece nearest `at` is trimmed away
/// where `cutters` cross it. An empty list means the whole object goes (it
/// can only happen when every piece is the clicked one, which a trim of two or
/// more pieces never leaves). `Err` says why nothing can be done.
pub fn trim_plan(
    target: &Entity,
    cutters: &[Entity],
    at: Point,
) -> Result<Vec<Entity>, &'static str> {
    if matches!(target, Entity::Spline(_)) {
        return Err("TRIM does not support splines yet.");
    }
    let crossings: Vec<Point> = cutters
        .iter()
        .flat_map(|cutter| target.intersections(cutter))
        .collect();
    if crossings.is_empty() {
        let near = target.bounds();
        if cutters
            .iter()
            .any(|c| matches!(c, Entity::Spline(_)) && c.bounds().intersects(&near))
        {
            return Err("Splines cannot be cutting edges yet.");
        }
        return Err("Object does not intersect an edge.");
    }
    let pieces = target.split_at(&crossings);
    // One piece means the cuts did not divide it (a circle cut once, a
    // closed shape cut once, a cut at an end): there is nothing to remove.
    if pieces.len() < 2 {
        return Err("Object cannot be trimmed there.");
    }
    let nearest = pieces
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.distance_to(at).total_cmp(&b.1.distance_to(at)))
        .map(|(i, _)| i)
        .ok_or("Object cannot be trimmed there.")?;
    Ok(pieces
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != nearest)
        .map(|(_, piece)| piece)
        .collect())
}

/// What extending `target` at the end nearest `at` does against `boundaries`:
/// the extended object. `Err` says why not.
pub fn extend_plan(
    target: &Entity,
    boundaries: &[Entity],
    at: Point,
) -> Result<Vec<Entity>, &'static str> {
    if matches!(target, Entity::Spline(_)) {
        return Err("EXTEND does not support splines yet.");
    }
    let ends = match target {
        Entity::Line { start, end } => Some((*start, *end)),
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => Some((
            od_core::arc::point_at(*center, *radius, *start),
            od_core::arc::point_at(*center, *radius, *end),
        )),
        Entity::Ellipse(e) if !e.is_full() => Some((e.start_point(), e.end_point())),
        Entity::Polyline {
            points,
            closed: false,
            ..
        } if points.len() >= 2 => Some((points[0], points[points.len() - 1])),
        _ => None,
    };
    let Some((first, last)) = ends else {
        return Err("Object cannot be extended.");
    };
    let gap = |p: Point| (p.x - at.x).hypot(p.y - at.y);
    let end = if gap(first) <= gap(last) {
        End::Start
    } else {
        End::End
    };
    let splines = boundaries.iter().any(|b| matches!(b, Entity::Spline(_)));
    target
        .extended_to(end, boundaries)
        .map(|longer| vec![longer])
        .ok_or(if splines {
            "No boundary in the way (splines cannot be boundary edges yet)."
        } else {
            "No boundary in the way."
        })
}

/// Which of the two commands is running.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Trim,
    Extend,
}

impl Mode {
    fn other(self) -> Mode {
        match self {
            Mode::Trim => Mode::Extend,
            Mode::Extend => Mode::Trim,
        }
    }
}

/// TRIM: choose cutting edges (Enter takes every object), then click the part
/// of each object to remove. Shift-click extends instead. Each change is its
/// own undo step; Undo takes back the last one; Enter or Esc ends.
pub fn trim_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    edit_against_edges(
        ed,
        Mode::Trim,
        "Select cutting edges or <select all>",
        "Select object to trim or shift-select to extend",
    )
}

/// EXTEND: choose boundary edges (Enter takes every object), then click each
/// object near the end to lengthen. Shift-click trims instead.
pub fn extend_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    edit_against_edges(
        ed,
        Mode::Extend,
        "Select boundary edges or <select all>",
        "Select object to extend or shift-select to trim",
    )
}

fn edit_against_edges(
    ed: &mut Editor,
    mode: Mode,
    edges_message: &str,
    object_message: &str,
) -> Result<(), Cancelled> {
    // Objects selected before the command are the edges: skip asking.
    let chosen = if ed.pickset().is_empty() {
        ed.ask_selection(Options::<()>::new(edges_message).allow_empty())?
    } else {
        let count = ed.pickset().len();
        ed.say(format!(
            "Using the {} selected as edges.",
            plural(count, "object")
        ));
        ed.pickset().to_vec()
    };
    // `None` stands for every object in the drawing.
    let edges = (!chosen.is_empty()).then_some(chosen);
    if edges.is_none() {
        ed.say("Using every object as an edge.");
    }
    let mut changed = 0_usize;
    loop {
        let mut options = Options::<Picked>::new(object_message);
        if changed > 0 {
            options = options.keyword("Undo", "U");
        }
        match ed.ask_object(options) {
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            Ok(Answer::Keyword(_)) => {
                ed.undo()?;
                changed -= 1;
            }
            Ok(Answer::Value(picked)) => {
                let doing = if picked.shift { mode.other() } else { mode };
                if change_one(ed, &edges, doing, picked)? {
                    changed += 1;
                }
            }
        }
    }
}

/// Trim or extend the picked object. Returns whether the drawing changed.
fn change_one(
    ed: &Editor,
    edges: &Option<Vec<EntityId>>,
    mode: Mode,
    picked: Picked,
) -> Result<bool, Cancelled> {
    let Picked { id, at, .. } = picked;
    let edges = edges.clone();
    let plan = ed.read(move |doc| {
        let target = doc.object(id)?;
        let others: Vec<Entity> = match &edges {
            Some(ids) => ids
                .iter()
                .filter(|other| **other != id)
                .filter_map(|other| doc.object(*other))
                .map(|o| o.entity.clone())
                .collect(),
            None => doc
                .visible_objects()
                .filter(|o| o.id != id)
                .map(|o| o.entity.clone())
                .collect(),
        };
        let made = match mode {
            Mode::Trim => trim_plan(&target.entity, &others, at),
            Mode::Extend => extend_plan(&target.entity, &others, at),
        };
        Some(made.map(|left| (target.attrs.clone(), left)))
    })?;
    let (attrs, mut left) = match plan {
        None => return Ok(false),
        Some(Err(why)) => {
            ed.say(why);
            return Ok(false);
        }
        Some(Ok(done)) => done,
    };
    let name = match mode {
        Mode::Trim => "Trim",
        Mode::Extend => "Extend",
    };
    ed.transaction(name, move |tx| {
        if left.is_empty() {
            tx.remove(vec![id]);
            return Ok(());
        }
        // The first piece keeps the object's identity and place in the draw
        // order; any others are new objects on the same layer.
        let first = left.remove(0);
        tx.modify(id, move |_| first);
        if !left.is_empty() {
            tx.add_with(left.into_iter().map(|e| (e, attrs.clone())).collect());
        }
        Ok(())
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn wall(x: f64) -> Entity {
        line(p(x, -10.0), p(x, 10.0))
    }

    #[test]
    fn the_clicked_piece_goes_and_the_rest_stays() {
        let target = line(p(0.0, 0.0), p(10.0, 0.0));
        let cutters = [wall(3.0), wall(7.0)];
        // Click the middle: the ends remain.
        assert_eq!(
            trim_plan(&target, &cutters, p(5.0, 0.0)).unwrap(),
            [
                line(p(0.0, 0.0), p(3.0, 0.0)),
                line(p(7.0, 0.0), p(10.0, 0.0))
            ]
        );
        // Click an end: it shortens.
        assert_eq!(
            trim_plan(&target, &cutters, p(9.0, 0.0)).unwrap(),
            [
                line(p(0.0, 0.0), p(3.0, 0.0)),
                line(p(3.0, 0.0), p(7.0, 0.0))
            ]
        );
    }

    #[test]
    fn ellipses_are_trimmed_extended_and_cut_against() {
        use std::f64::consts::{PI, TAU};
        let ellipse = |start: f64, end: f64| {
            Entity::Ellipse(
                od_core::Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, start, end).unwrap(),
            )
        };
        // A full ellipse cut by the y axis: the side clicked goes.
        let kept = trim_plan(&ellipse(0.0, TAU), &[wall(0.0)], p(4.0, 0.0)).unwrap();
        let [Entity::Ellipse(left)] = kept.as_slice() else {
            panic!("one elliptical arc, got {kept:?}")
        };
        assert!(left.start_point().x.abs() < 1e-9 && left.mid_point().x < -3.9);
        // A line across the ellipse is trimmed back to it.
        let across = line(p(-10.0, 0.0), p(10.0, 0.0));
        let kept = trim_plan(&across, &[ellipse(0.0, TAU)], p(0.0, 0.0)).unwrap();
        assert_eq!(kept.len(), 2, "the part inside goes");
        // The upper half grows clockwise from its start (the major end) down to x = 3.
        let grown = extend_plan(&ellipse(0.0, PI), &[wall(3.0)], p(4.0, 0.1)).unwrap();
        let [Entity::Ellipse(e)] = grown.as_slice() else {
            panic!("an elliptical arc, got {grown:?}")
        };
        assert!((e.start_point().x - 3.0).abs() < 1e-9 && e.start_point().y < 0.0);
        // A line grows to the ellipse.
        let short = line(p(-10.0, 0.0), p(-8.0, 0.0));
        let grown = extend_plan(&short, &[ellipse(0.0, TAU)], p(-8.0, 0.0)).unwrap();
        assert_eq!(grown, vec![line(p(-10.0, 0.0), p(-4.0, 0.0))]);
        assert_eq!(
            extend_plan(&ellipse(0.0, TAU), &[wall(9.0)], p(4.0, 0.0)),
            Err("Object cannot be extended.")
        );
    }

    #[test]
    fn splines_are_refused_by_name_as_objects_and_as_edges() {
        let s = Entity::Spline(Box::new(
            od_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)]).unwrap(),
        ));
        assert_eq!(
            trim_plan(&s, &[wall(0.0)], p(1.0, 0.0)),
            Err("TRIM does not support splines yet.")
        );
        assert_eq!(
            extend_plan(&s, &[wall(9.0)], p(4.0, 0.0)),
            Err("EXTEND does not support splines yet.")
        );
        let across = line(p(0.0, -5.0), p(0.0, 5.0));
        assert_eq!(
            trim_plan(&across, std::slice::from_ref(&s), p(0.0, 1.0)),
            Err("Splines cannot be cutting edges yet.")
        );
        let short = line(p(0.0, -5.0), p(0.0, -3.0));
        assert_eq!(
            extend_plan(&short, &[s], p(0.0, -3.0)),
            Err("No boundary in the way (splines cannot be boundary edges yet).")
        );
    }

    #[test]
    fn nothing_to_cut_against_or_no_division_is_explained() {
        let target = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            trim_plan(&target, &[], p(5.0, 0.0)),
            Err("Object does not intersect an edge.")
        );
        assert_eq!(
            trim_plan(&target, &[wall(50.0)], p(5.0, 0.0)),
            Err("Object does not intersect an edge.")
        );
        // A cutter touching only the end does not divide the line.
        assert_eq!(
            trim_plan(&target, &[wall(10.0)], p(5.0, 0.0)),
            Err("Object cannot be trimmed there.")
        );
        // A circle needs two cuts.
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let tangent = wall(2.0);
        assert_eq!(
            trim_plan(&circle, &[tangent], p(2.0, 0.0)),
            Err("Object cannot be trimmed there.")
        );
    }

    #[test]
    fn a_circle_trims_to_an_arc() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        // A vertical cutter through the middle cuts it top and bottom;
        // clicking the right-hand side leaves the left half as an arc.
        let left = trim_plan(&circle, &[wall(0.0)], p(2.0, 0.0)).unwrap();
        assert_eq!(left.len(), 1);
        let Entity::Arc { .. } = left[0] else {
            panic!("a circle trims to an arc, got {:?}", left[0]);
        };
        assert!(left[0].distance_to(p(-2.0, 0.0)) < 1e-9);
        assert!(
            left[0].distance_to(p(2.0, 0.0)) > 1.0,
            "the right side is gone"
        );
    }

    /// Start TRIM with objects already in the drawing.
    fn trimming(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Trim", trim_objects);
        state.settle_command();
        state
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn id(state: &AppState, index: usize) -> EntityId {
        state.session().document().objects()[index].id
    }

    #[test]
    fn enter_takes_every_object_as_an_edge_and_each_click_is_one_undo_step() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(10.0, 0.0)), wall(4.0), wall(6.0)]);
        state.answer_enter();
        state.settle_command();
        assert!(state.cmdline.log.iter().any(|l| l.contains("every object")));
        assert!(state.cmd_prompt().text.starts_with("Select object to trim"));
        let target = id(&state, 0);
        state.answer_object(p(5.0, 0.0), Some(target), false);
        state.settle_command();
        assert_eq!(
            entities(&state)[0],
            line(p(0.0, 0.0), p(4.0, 0.0)),
            "the first piece keeps the object"
        );
        assert!(entities(&state).contains(&line(p(6.0, 0.0), p(10.0, 0.0))));
        assert_eq!(
            entities(&state).len(),
            4,
            "two walls and the two pieces left"
        );
        // Undo (the keyword) takes back that one trim.
        assert!(state.cmd_prompt().options.iter().any(|o| o.label == "Undo"));
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(10.0, 0.0)));
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn objects_selected_beforehand_are_the_edges_and_the_first_question_is_skipped() {
        let mut state = AppState::new();
        for e in [line(p(0.0, 0.0), p(10.0, 0.0)), wall(4.0), wall(6.0)] {
            state.add_entity(e);
        }
        let near = id(&state, 1);
        state.select_ids([near], false);
        state.start_command("Trim", trim_objects);
        state.settle_command();
        // Straight to choosing what to trim, using only the selected wall.
        assert!(state.cmd_prompt().text.starts_with("Select object to trim"));
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("1 object selected")));
        let target = id(&state, 0);
        state.answer_object(p(8.0, 0.0), Some(target), false);
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(4.0, 0.0)));
        // Extend skips it the same way.
        state.cancel();
        state.cancel();
        state.select_ids([id(&state, 2)], false);
        state.start_command("Extend", extend_objects);
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to extend"));
    }

    #[test]
    fn only_the_chosen_edges_cut() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(10.0, 0.0)), wall(4.0), wall(6.0)]);
        let near = id(&state, 1);
        state.select_ids([near], false);
        state.answer_enter();
        state.settle_command();
        let target = id(&state, 0);
        // Click the right-hand part: only the wall at 4 cuts, so the part
        // from 4 to the end goes.
        state.answer_object(p(8.0, 0.0), Some(target), false);
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(4.0, 0.0)));
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn an_object_that_no_edge_crosses_is_left_alone_and_the_user_told() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(10.0, 0.0)), wall(40.0)]);
        state.answer_enter();
        state.settle_command();
        let target = id(&state, 0);
        state.answer_object(p(5.0, 0.0), Some(target), false);
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("does not intersect"));
        assert!(state.command_running(), "still asking for the next object");
        // A click on nothing does nothing.
        state.answer_object(p(1.0, 1.0), None, false);
        state.settle_command();
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running(), "Enter ends it");
    }

    #[test]
    fn extend_plan_lengthens_the_end_nearest_the_click() {
        let target = line(p(0.0, 0.0), p(2.0, 0.0));
        let walls = [wall(5.0), wall(-3.0)];
        assert_eq!(
            extend_plan(&target, &walls, p(1.9, 0.0)).unwrap(),
            [line(p(0.0, 0.0), p(5.0, 0.0))]
        );
        assert_eq!(
            extend_plan(&target, &walls, p(0.1, 0.0)).unwrap(),
            [line(p(-3.0, 0.0), p(2.0, 0.0))]
        );
        assert_eq!(
            extend_plan(&target, &[wall(-50.0)], p(1.9, 0.0)),
            Err("No boundary in the way.")
        );
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(
            extend_plan(&circle, &walls, p(1.0, 0.0)),
            Err("Object cannot be extended.")
        );
    }

    fn extending(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Extend", extend_objects);
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        state
    }

    #[test]
    fn extend_grows_an_object_and_shift_trims_it_instead() {
        let mut state = extending(&[line(p(0.0, 0.0), p(2.0, 0.0)), wall(6.0), wall(1.0)]);
        let target = id(&state, 0);
        // Near the right end: grows to the wall at 6 (the wall at 1 is behind).
        state.answer_object(p(1.9, 0.0), Some(target), false);
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(6.0, 0.0)));
        // Shift-click trims instead: the wall at 1 cuts the line and the
        // part clicked at 4 goes, leaving 0 to 1.
        state.answer_object(p(4.0, 0.0), Some(target), true);
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(1.0, 0.0)));
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn shift_extends_while_trimming() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(2.0, 0.0)), wall(6.0)]);
        state.answer_enter();
        state.settle_command();
        let target = id(&state, 0);
        state.answer_object(p(1.9, 0.0), Some(target), true);
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(6.0, 0.0)));
    }
}
