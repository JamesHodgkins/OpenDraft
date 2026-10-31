//! FILLET and CHAMFER: join two objects at a corner with an arc or a cut, or
//! round the corners of a polyline. Written against the `Editor`; the
//! geometry is `fillet`, `chamfer_lines` and `round_polyline` in `od_core`.

use od_commands::{Answer, Cancelled, Editor, Options, Picked};
use od_core::{chamfer_lines, fillet, round_polyline, Chamfer, Entity, EntityId, Joined, Round};

use crate::corner_memory::Memory;
use crate::defaults::EditorDefaults;
use crate::text::plural;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fillet,
    Chamfer,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Fillet => "Fillet",
            Kind::Chamfer => "Chamfer",
        }
    }
}

/// FILLET: round the corner of two objects (lines, arcs, circles, construction lines, rays) with an arc
/// of the current radius, or two segments of one polyline. Options set the
/// radius, whether the objects are trimmed back to the arc, round every
/// vertex of a polyline (Polyline), and carry on with more corners (Multiple).
/// Shift on the second pick joins them at a sharp corner instead.
pub fn fillet_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    join_corners(ed, Kind::Fillet)
}

/// CHAMFER: cut the corner of two lines (or two segments of a polyline) with
/// a straight line, by two distances or by a length and an angle. Options as
/// for FILLET. Shift on the second pick joins them at a sharp corner.
pub fn chamfer_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    join_corners(ed, Kind::Chamfer)
}

fn settings_text(kind: Kind, m: &Memory) -> String {
    let mode = if m.trim { "Trim" } else { "No trim" };
    match kind {
        Kind::Fillet => format!("({mode} mode, Radius = {})", m.radius),
        Kind::Chamfer if m.by_angle => {
            format!("({mode} mode, Length = {}, Angle = {})", m.length, m.angle)
        }
        Kind::Chamfer => format!("({mode} mode, Dist1 = {}, Dist2 = {})", m.first, m.second),
    }
}

fn join_corners(ed: &mut Editor, kind: Kind) -> Result<(), Cancelled> {
    let mut multiple = false;
    loop {
        let m = ed.defaults().corner.get();
        let mut options =
            Options::<Picked>::new(format!("{} Select first object", settings_text(kind, &m)));
        options = match kind {
            Kind::Fillet => options.keyword("Radius", "R"),
            Kind::Chamfer => options.keyword("Distance", "D").keyword("Angle", "A"),
        };
        options = options.keyword("Polyline", "P").keyword("Trim", "T");
        if !multiple {
            options = options.keyword("Multiple", "M");
        }
        match ed.ask_object(options) {
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            Ok(Answer::Keyword(k)) => match k.as_str() {
                "Polyline" => {
                    if round_whole_polyline(ed, kind)? && !multiple {
                        return Ok(());
                    }
                }
                "Multiple" => multiple = true,
                _ => change_setting(ed, &k, &m)?,
            },
            Ok(Answer::Value(first)) => {
                if !join_second(ed, kind, first)? {
                    return Ok(());
                }
                if !multiple {
                    return Ok(());
                }
            }
        }
    }
}

/// Handle an option that changes a setting.
fn change_setting(ed: &Editor, keyword: &str, m: &Memory) -> Result<(), Cancelled> {
    match keyword {
        "Radius" => {
            let r =
                ed.get_distance(Options::<f64>::new("Specify fillet radius").default(m.radius))?;
            ed.defaults().corner.change(|m| m.radius = r.max(0.0));
        }
        "Distance" => {
            let first = ed.get_distance(
                Options::<f64>::new("Specify first chamfer distance").default(m.first),
            )?;
            let second = ed.get_distance(
                Options::<f64>::new("Specify second chamfer distance").default(first),
            )?;
            ed.defaults().corner.change(|m| {
                m.first = first.max(0.0);
                m.second = second.max(0.0);
                m.by_angle = false;
            });
        }
        "Angle" => {
            let length = ed.get_distance(
                Options::<f64>::new("Specify chamfer length on the first line").default(m.length),
            )?;
            let angle = ed.get_angle(
                Options::<f64>::new("Specify chamfer angle from the first line")
                    .default(m.angle.to_radians()),
            )?;
            ed.defaults().corner.change(|m| {
                m.length = length.max(0.0);
                m.angle = angle.to_degrees();
                m.by_angle = true;
            });
        }
        "Trim" => {
            let current = if m.trim { "Trim" } else { "No trim" };
            let answer = ed.get_keyword(
                Options::<String>::new("Trim mode")
                    .keyword("Trim", "T")
                    .keyword("No trim", "N")
                    .default(current.to_owned()),
            )?;
            ed.defaults().corner.change(|m| m.trim = answer == "Trim");
        }
        _ => {}
    }
    Ok(())
}

/// Ask for the second object and join. Returns whether to carry on (`false`
/// when the user ended the command).
fn join_second(ed: &Editor, kind: Kind, first: Picked) -> Result<bool, Cancelled> {
    loop {
        let second = match ed.ask_object(Options::<Picked>::new("Select second object")) {
            Err(Cancelled) | Ok(Answer::Enter) | Ok(Answer::Keyword(_)) => return Ok(false),
            Ok(Answer::Value(second)) => second,
        };
        match join_pair(ed, kind, first, second)? {
            Outcome::Retry => continue,
            Outcome::Finished => return Ok(true),
        }
    }
}

enum Outcome {
    /// Ask for the second object again.
    Retry,
    Finished,
}

/// What joining two picked objects does to the drawing.
enum Plan {
    /// Two objects and (maybe) a joint between them.
    Pair(Box<Joined>),
    /// One polyline replaced by these pieces (it stays one polyline).
    Replace(Vec<Entity>),
}

fn plan_pair(
    m: &Memory,
    kind: Kind,
    a: &Entity,
    b: &Entity,
    same: bool,
    first: Picked,
    second: Picked,
) -> Result<Plan, Option<String>> {
    if kind == Kind::Fillet && [a, b].iter().any(|e| matches!(e, Entity::Spline(_))) {
        return Err(Some("FILLET does not support splines yet.".to_owned()));
    }
    let sharp = second.shift;
    if same {
        // Two segments of one polyline.
        let Some(path) = a.polyline_path() else {
            return Err(None);
        };
        let (Some(i), Some(j)) = (a.segment_at(first.at), a.segment_at(second.at)) else {
            return Err(Some(
                "Select two adjacent segments of the polyline.".to_owned(),
            ));
        };
        let vertex = a
            .shared_vertex(i, j)
            .ok_or_else(|| Some("Select two adjacent segments of the polyline.".to_owned()))?;
        let how = match kind {
            Kind::Fillet => Round::Fillet(if sharp { 0.0 } else { m.radius }),
            Kind::Chamfer if sharp => Round::Chamfer(Chamfer::Distances(0.0, 0.0)),
            Kind::Chamfer => Round::Chamfer(m.chamfer()),
        };
        return corner_polyline(&path, how, Some(vertex));
    }
    let joined = match kind {
        Kind::Fillet => fillet(
            a,
            b,
            if sharp { 0.0 } else { m.radius },
            first.at,
            second.at,
        ),
        Kind::Chamfer => {
            let how = if sharp {
                Chamfer::Distances(0.0, 0.0)
            } else {
                m.chamfer()
            };
            chamfer_lines(a, b, how, first.at, second.at)
        }
    };
    joined
        .map(|j| Plan::Pair(Box::new(j)))
        .map_err(|e| Some(e.to_string()))
}

fn corner_polyline(
    path: &od_core::Path,
    how: Round,
    only: Option<usize>,
) -> Result<Plan, Option<String>> {
    let done = round_polyline(path, how, only).map_err(|e| Some(e.to_string()))?;
    Ok(Plan::Replace(done.pieces))
}

fn join_pair(ed: &Editor, kind: Kind, first: Picked, second: Picked) -> Result<Outcome, Cancelled> {
    let ids = (first.id, second.id);
    let read = ed.read(move |doc| {
        let (a, b) = (doc.object(ids.0)?, doc.object(ids.1)?);
        Some((a.entity.clone(), b.entity.clone(), a.attrs.clone()))
    })?;
    let Some((a, b, attrs)) = read else {
        return Ok(Outcome::Finished);
    };
    let same = first.id == second.id;
    let plan = match plan_pair(
        &ed.defaults().corner.get(),
        kind,
        &a,
        &b,
        same,
        first,
        second,
    ) {
        Ok(plan) => plan,
        Err(None) => {
            ed.say("Select a different object.");
            return Ok(Outcome::Retry);
        }
        Err(Some(why)) => {
            ed.say(why);
            return Ok(Outcome::Finished);
        }
    };
    apply(ed, kind, first.id, second.id, attrs, plan)?;
    Ok(Outcome::Finished)
}

/// Make the change in the drawing, as one undo step.
fn apply(
    ed: &Editor,
    kind: Kind,
    id_a: EntityId,
    id_b: EntityId,
    attrs: od_core::Attributes,
    plan: Plan,
) -> Result<(), Cancelled> {
    let trim = ed.defaults().corner.get().trim;
    match plan {
        Plan::Pair(joined) => {
            if !trim && joined.joint.is_none() {
                ed.say("Nothing to do: no trim and no joining piece.");
                return Ok(());
            }
            ed.transaction(kind.name(), move |tx| {
                if trim {
                    let (new_a, new_b) = (joined.a, joined.b);
                    tx.modify(id_a, move |_| new_a);
                    tx.modify(id_b, move |_| new_b);
                }
                if let Some(joint) = joined.joint {
                    tx.add_with(vec![(joint, attrs)]);
                }
                Ok(())
            })
        }
        Plan::Replace(mut pieces) => {
            if pieces.is_empty() {
                return Ok(());
            }
            ed.transaction(kind.name(), move |tx| {
                let first = pieces.remove(0);
                tx.modify(id_a, move |_| first);
                if !pieces.is_empty() {
                    tx.add_with(pieces.into_iter().map(|e| (e, attrs.clone())).collect());
                }
                Ok(())
            })
        }
    }
}

/// The Polyline option: pick one polyline and round every corner that fits.
/// Returns whether something was done.
fn round_whole_polyline(ed: &Editor, kind: Kind) -> Result<bool, Cancelled> {
    let picked = match ed.ask_object(Options::<Picked>::new("Select polyline")) {
        Ok(Answer::Value(picked)) => picked,
        Err(Cancelled) | Ok(_) => return Ok(false),
    };
    let id = picked.id;
    let read = ed.read(move |doc| doc.object(id).map(|o| (o.entity.clone(), o.attrs.clone())))?;
    let Some((entity, attrs)) = read else {
        return Ok(false);
    };
    let Some(path) = entity.polyline_path() else {
        ed.say("That is not a polyline.");
        return Ok(false);
    };
    let m = ed.defaults().corner.get();
    let how = match kind {
        Kind::Fillet => Round::Fillet(m.radius),
        Kind::Chamfer => Round::Chamfer(m.chamfer()),
    };
    let done = match round_polyline(&path, how, None) {
        Ok(done) => done,
        Err(e) => {
            ed.say(e.to_string());
            return Ok(false);
        }
    };
    if done.done == 0 {
        ed.say("No corner could be done.");
        return Ok(false);
    }
    ed.say(format!(
        "{} done{}.",
        plural(done.done, "corner"),
        if done.skipped > 0 {
            format!(
                ", {} left sharp (segments too short)",
                plural(done.skipped, "corner")
            )
        } else {
            String::new()
        }
    ));
    apply(ed, kind, id, id, attrs, Plan::Replace(done.pieces))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn click_with(state: &mut AppState, index: usize, at: Point, shift: bool) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), shift);
        state.settle_command();
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        click_with(state, index, at, false);
    }

    fn started(
        objects: &[Entity],
        command: fn(&mut Editor) -> Result<(), Cancelled>,
        inputs: &[&str],
    ) -> AppState {
        let mut state = AppState::new();
        for e in objects {
            state.add_entity(e.clone());
        }
        state.start_command("Corner", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        state
    }

    /// Two lines meeting at the origin along the axes.
    fn axes() -> Vec<Entity> {
        vec![
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(0.0, 0.0), p(0.0, 10.0)),
        ]
    }

    fn square() -> Entity {
        Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    #[test]
    fn fillet_and_chamfer_cut_construction_lines_and_rays_back_to_rays() {
        let objects = [
            Entity::Xline {
                origin: p(0.0, 0.0),
                direction: p(1.0, 0.0),
            },
            Entity::Ray {
                origin: p(0.0, -5.0),
                direction: p(0.0, 1.0),
            },
        ];
        let mut state = started(&objects, fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(8.0, 0.0));
        click(&mut state, 1, p(0.0, 8.0));
        let all = entities(&state);
        assert_eq!(all.len(), 3, "the arc is new");
        let (origin, _, ray) = all[0].infinite_line().unwrap();
        assert!(ray, "a construction line cut at one end is a ray");
        assert!((origin.x - 2.0).abs() < 1e-6);
        let (origin, _, ray) = all[1].infinite_line().unwrap();
        assert!(ray && (origin.y - 2.0).abs() < 1e-6);
        assert!(matches!(all[2], Entity::Arc { .. }));
        state.undo();
        assert_eq!(entities(&state).len(), 2, "one undo step");
        assert!(matches!(entities(&state)[0], Entity::Xline { .. }));
    }

    #[test]
    fn fillet_rounds_the_corner_and_trims_both_lines() {
        let mut state = started(&axes(), fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 5.0));
        assert!(!state.command_running(), "one corner per command");
        let all = entities(&state);
        assert_eq!(all.len(), 3);
        let Entity::Line { start, .. } = all[0] else {
            panic!()
        };
        assert!((start.x - 2.0).abs() < 1e-9 && start.y.abs() < 1e-9);
        let Entity::Arc { center, radius, .. } = all[2] else {
            panic!("the third object is the arc, got {:?}", all[2]);
        };
        assert!((center.x - 2.0).abs() < 1e-9 && (center.y - 2.0).abs() < 1e-9);
        assert!((radius - 2.0).abs() < 1e-9);
        state.undo();
        assert_eq!(
            entities(&state),
            axes(),
            "one undo step for the whole fillet"
        );
    }

    #[test]
    fn an_ellipse_is_filleted_but_not_chamfered() {
        let half = od_core::Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, std::f64::consts::PI);
        let items = vec![
            Entity::Ellipse(half.unwrap()),
            line(p(3.0, -5.0), p(3.0, 5.0)),
        ];
        let mut state = started(&items, fillet_objects, &["r", "0.5"]);
        click(&mut state, 0, p(-2.0, 1.73));
        click(&mut state, 1, p(3.0, 4.0));
        let all = entities(&state);
        assert_eq!(
            all.len(),
            3,
            "the arc was cut back and a fillet added: {all:?}"
        );
        assert!(matches!(all[0], Entity::Ellipse(e) if e.start_point().x < 3.0));
        assert!(matches!(all[2], Entity::Arc { .. }));
        // As in AutoCAD, a chamfer needs lines.
        let mut state = started(&items, chamfer_objects, &[]);
        click(&mut state, 0, p(-2.0, 1.73));
        click(&mut state, 1, p(3.0, 4.0));
        assert_eq!(entities(&state), items);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("A chamfer needs lines")));
    }

    #[test]
    fn a_spline_is_not_filleted() {
        let items = vec![
            axes()[0].clone(),
            Entity::Spline(Box::new(
                od_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)])
                    .unwrap()
                    .translated(0.0, 10.0),
            )),
        ];
        let mut state = started(&items, fillet_objects, &[]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 10.0));
        assert_eq!(entities(&state), items);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("FILLET does not support splines yet.")));
    }

    #[test]
    fn no_trim_adds_the_arc_and_leaves_the_lines() {
        let mut state = started(&axes(), fillet_objects, &["r", "2", "t", "n"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 5.0));
        let all = entities(&state);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], axes()[0]);
    }

    #[test]
    fn shift_on_the_second_pick_joins_at_a_sharp_corner() {
        let lines = vec![
            line(p(0.0, 0.0), p(5.0, 0.0)),
            line(p(8.0, -6.0), p(8.0, 4.0)),
        ];
        let mut state = started(&lines, fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(3.0, 0.0));
        click_with(&mut state, 1, p(8.0, -4.0), true);
        let all = entities(&state);
        assert_eq!(all.len(), 2, "no arc");
        assert_eq!(all[0], line(p(0.0, 0.0), p(8.0, 0.0)));
    }

    #[test]
    fn a_fillet_between_a_line_and_a_circle() {
        let objects = vec![
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 5.0,
            },
            line(p(-20.0, -8.0), p(20.0, -8.0)),
        ];
        let mut state = started(&objects, fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(12.0, -8.0));
        let all = entities(&state);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], objects[0], "the circle is left whole");
        assert!(matches!(all[2], Entity::Arc { .. }));
    }

    #[test]
    fn a_fillet_that_cannot_be_made_is_explained() {
        let far = vec![
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 1.0,
            },
            Entity::Circle {
                center: p(100.0, 0.0),
                radius: 1.0,
            },
        ];
        let mut state = started(&far, fillet_objects, &["r", "0.1"]);
        click(&mut state, 0, p(1.0, 0.0));
        click(&mut state, 1, p(99.0, 0.0));
        assert_eq!(entities(&state).len(), 2);
        assert!(state.cmdline.log.last().unwrap().contains("No arc"));
    }

    #[test]
    fn the_same_line_twice_is_refused_and_the_second_asked_again() {
        let mut state = started(&axes(), fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 0, p(6.0, 0.0));
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("different object"));
        assert!(state.cmd_prompt().text.starts_with("Select second object"));
    }

    #[test]
    fn two_segments_of_one_polyline_are_filleted_at_their_shared_vertex() {
        let mut state = started(&[square()], fillet_objects, &["r", "2"]);
        // The bottom side, then the right side: the corner at (10, 0).
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 0, p(10.0, 5.0));
        // The polyline stays one object, now with one arc segment in it.
        let all = entities(&state);
        assert_eq!(all.len(), 1);
        let Entity::Polyline {
            points,
            bulges,
            closed,
            ..
        } = &all[0]
        else {
            panic!("still a polyline")
        };
        assert!(*closed && points.len() == 5);
        assert_eq!(bulges.iter().filter(|b| b.abs() > 1e-9).count(), 1);
        // Opposite sides do not meet.
        let mut state = started(&[square()], fillet_objects, &["r", "2"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 0, p(5.0, 10.0));
        assert!(state.cmdline.log.last().unwrap().contains("adjacent"));
        assert_eq!(entities(&state), vec![square()]);
    }

    #[test]
    fn the_polyline_option_rounds_every_corner() {
        let mut state = started(&[square()], fillet_objects, &["r", "2", "p"]);
        click(&mut state, 0, p(5.0, 0.0));
        assert!(!state.command_running());
        let all = entities(&state);
        assert_eq!(all.len(), 1, "one polyline, not lines and arcs");
        let Entity::Polyline {
            points,
            bulges,
            closed,
            ..
        } = &all[0]
        else {
            panic!()
        };
        assert!(*closed && points.len() == 8);
        assert_eq!(bulges.iter().filter(|b| b.abs() > 1e-9).count(), 4);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("4 corners done")));
        state.undo();
        assert_eq!(entities(&state), vec![square()], "one undo step");
    }

    #[test]
    fn a_chamfer_by_two_distances_cuts_the_corner() {
        let mut state = started(&axes(), chamfer_objects, &["d", "2", "3"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 5.0));
        let all = entities(&state);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2], line(p(2.0, 0.0), p(0.0, 3.0)));
    }

    #[test]
    fn a_chamfer_by_length_and_angle() {
        let mut state = started(&axes(), chamfer_objects, &["a", "3", "45"]);
        assert!(state.defaults().corner.get().by_angle);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 5.0));
        let Entity::Line { start, end } = entities(&state)[2] else {
            panic!()
        };
        assert!((start.x - 3.0).abs() < 1e-9 && (end.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_polyline_option_chamfers_every_corner_into_one_polyline() {
        let mut state = started(&[square()], chamfer_objects, &["d", "1", "1", "p"]);
        click(&mut state, 0, p(5.0, 0.0));
        let all = entities(&state);
        assert_eq!(all.len(), 1, "a chamfer keeps the polyline whole");
        assert!(
            matches!(&all[0], Entity::Polyline { points, closed: true, .. } if points.len() == 8)
        );
    }

    #[test]
    fn multiple_keeps_asking_until_enter() {
        let mut state = started(&axes(), fillet_objects, &["r", "1", "m"]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(0.0, 5.0));
        assert!(
            state.command_running(),
            "Multiple: ready for the next corner"
        );
        assert!(state.cmd_prompt().text.starts_with("(Trim mode"));
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running());
    }

    #[test]
    fn the_settings_are_remembered_for_next_time() {
        let mut state = started(&axes(), fillet_objects, &["r", "7"]);
        assert_eq!(state.defaults().corner.get().radius, 7.0);
        state.cancel();
        state.start_command("Corner", fillet_objects);
        state.settle_command();
        assert!(state.cmd_prompt().text.contains("Radius = 7"));
    }
}
