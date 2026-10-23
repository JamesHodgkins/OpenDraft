//! XLINE and RAY: lines with no ends, or no far end. The geometry is
//! `od_core::infinite`; these only ask.
//!
//! XLINE takes a point and then through points (one line each, until Enter),
//! or Hor and Ver (through points), Ang (an angle, then through points),
//! Bisect (a vertex and the two sides of an angle) or Offset (parallel to a
//! line, construction line or ray). RAY takes a start and then through
//! points. Ang's Reference option is not built.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked, PointOptions};
use od_core::{infinite, Entity, Point};

use crate::draw::{add, distance, MIN_SIZE};

const NAME: &str = "Construction line";

fn xline(origin: Point, direction: Point) -> Entity {
    Entity::Xline { origin, direction }
}

fn ray(origin: Point, direction: Point) -> Entity {
    Entity::Ray { origin, direction }
}

/// The unit direction from `from` to `to`, or `None` when they coincide.
fn toward(from: Point, to: Point) -> Option<Point> {
    (distance(from, to) > MIN_SIZE)
        .then(|| infinite::unit(Point::new(to.x - from.x, to.y - from.y)))
        .flatten()
}

/// XLINE.
pub fn draw_xline(ed: &mut Editor) -> Result<(), Cancelled> {
    let options = PointOptions::new("Specify a point")
        .keyword("Hor", "H")
        .keyword("Ver", "V")
        .keyword("Ang", "A")
        .keyword("Bisect", "B")
        .keyword("Offset", "O")
        .ghost(|_| Vec::new());
    match ed.ask_point(options)? {
        Answer::Value(base) => through(ed, base, xline, NAME),
        Answer::Keyword(k) => match k.as_str() {
            "Hor" => parallel(ed, Point::new(1.0, 0.0)),
            "Ver" => parallel(ed, Point::new(0.0, 1.0)),
            "Ang" => {
                let angle =
                    ed.get_angle(NumberOptions::new("Enter angle of xline (0)").default(0.0))?;
                parallel(ed, infinite::from_angle(angle))
            }
            "Bisect" => bisect(ed),
            _ => offset(ed),
        },
        Answer::Enter => Ok(()),
    }
}

/// RAY.
pub fn draw_ray(ed: &mut Editor) -> Result<(), Cancelled> {
    let start = ed.get_point("Specify start point")?;
    through(ed, start, ray, "Ray")
}

/// Lines (made by `make`) from `base` through each point given, until Enter.
fn through(
    ed: &Editor,
    base: Point,
    make: fn(Point, Point) -> Entity,
    name: &'static str,
) -> Result<(), Cancelled> {
    loop {
        let options = PointOptions::new("Specify through point")
            .base(base)
            .ghost(move |at| {
                toward(base, at)
                    .map(|d| make(base, d))
                    .into_iter()
                    .collect()
            });
        match ed.ask_point(options) {
            Ok(Answer::Value(at)) => match toward(base, at) {
                Some(direction) => add(ed, make(base, direction), name)?,
                None => ed.say("The through point is the same as the first."),
            },
            Ok(_) | Err(Cancelled) => return Ok(()),
        }
    }
}

/// Construction lines along `direction` through each point given.
fn parallel(ed: &Editor, direction: Point) -> Result<(), Cancelled> {
    loop {
        let options =
            PointOptions::new("Specify through point").ghost(move |at| vec![xline(at, direction)]);
        match ed.ask_point(options) {
            Ok(Answer::Value(at)) => add(ed, xline(at, direction), NAME)?,
            Ok(_) | Err(Cancelled) => return Ok(()),
        }
    }
}

/// The line that halves an angle: its vertex and one point on each side. The
/// end of the second side is asked again for more lines at the same angle
/// start.
fn bisect(ed: &Editor) -> Result<(), Cancelled> {
    let vertex = ed.get_point("Specify angle vertex point")?;
    let start = loop {
        let at = ed.get_point(PointOptions::new("Specify angle start point").base(vertex))?;
        if toward(vertex, at).is_some() {
            break at;
        }
        ed.say("The point is the same as the vertex.");
    };
    let first = toward(vertex, start).unwrap_or(Point::new(1.0, 0.0));
    loop {
        let options = PointOptions::new("Specify angle end point")
            .base(vertex)
            .ghost(move |at| {
                toward(vertex, at)
                    .and_then(|second| infinite::bisector(first, second))
                    .map(|d| xline(vertex, d))
                    .into_iter()
                    .collect()
            });
        match ed.ask_point(options) {
            Ok(Answer::Value(end)) => {
                let half = toward(vertex, end).and_then(|second| infinite::bisector(first, second));
                match half {
                    Some(d) => add(ed, xline(vertex, d), NAME)?,
                    None => ed.say("The two sides do not make an angle."),
                }
            }
            Ok(_) | Err(Cancelled) => return Ok(()),
        }
    }
}

/// The direction of a line, construction line or ray; `None` for anything
/// else.
fn direction_of(entity: &Entity) -> Option<Point> {
    match entity {
        Entity::Line { start, end } => toward(*start, *end),
        other => other.infinite_line().map(|(_, direction, _)| direction),
    }
}

/// A point on a line, construction line or ray.
fn point_on(entity: &Entity) -> Option<Point> {
    match entity {
        Entity::Line { start, .. } => Some(*start),
        other => other.infinite_line().map(|(origin, ..)| origin),
    }
}

/// Construction lines parallel to an object, at a distance or through a
/// point, on the side picked.
fn offset(ed: &Editor) -> Result<(), Cancelled> {
    let options =
        NumberOptions::new("Specify offset distance or [Through]").keyword("Through", "T");
    let distance = match ed.ask_distance(options)? {
        Answer::Value(d) if d > MIN_SIZE => Some(d),
        Answer::Value(_) => {
            ed.say("Requires a positive distance.");
            return Ok(());
        }
        Answer::Keyword(_) | Answer::Enter => None,
    };
    loop {
        let Ok(Answer::Value(Picked { id, .. })) =
            ed.ask_object(Options::<Picked>::new("Select a line object or <exit>"))
        else {
            return Ok(());
        };
        let Some(entity) = ed.read(move |doc| doc.get(id).cloned())? else {
            continue;
        };
        let (Some(direction), Some(on)) = (direction_of(&entity), point_on(&entity)) else {
            ed.say("Select a line, construction line or ray.");
            continue;
        };
        let prompt = if distance.is_some() {
            "Specify side to offset"
        } else {
            "Specify through point"
        };
        let made = move |at: Point| -> Option<Entity> {
            // Which side `at` is on, and how far from the line.
            let side = (at.x - on.x) * -direction.y + (at.y - on.y) * direction.x;
            let moved = match distance {
                Some(d) if side.abs() > MIN_SIZE => {
                    infinite::shifted(on, direction, d * side.signum())
                }
                Some(_) => return None,
                None => at,
            };
            Some(xline(moved, direction))
        };
        let options = PointOptions::new(prompt).ghost(move |at| made(at).into_iter().collect());
        if let Answer::Value(at) = ed.ask_point(options)? {
            match made(at) {
                Some(line) => add(ed, line, NAME)?,
                None => ed.say("The point is on the line."),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn lines(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn run(command: fn(&mut Editor) -> Result<(), Cancelled>, inputs: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("XLINE", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        state
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-9
    }

    #[test]
    fn through_points_make_one_line_each_from_the_first_point() {
        let state = run(draw_xline, &["0,0", "10,0", "0,5"]);
        let found = lines(&state);
        assert_eq!(found.len(), 2);
        let (o, d, ray) = found[1].infinite_line().unwrap();
        assert!(near(o, Point::new(0.0, 0.0)) && near(d, Point::new(0.0, 1.0)) && !ray);
        assert!(state.command_running(), "asks for the next through point");
    }

    #[test]
    fn hor_and_ver_run_along_the_axes_through_each_point() {
        let state = run(draw_xline, &["h", "3,4", "7,8"]);
        let found = lines(&state);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], xline(Point::new(3.0, 4.0), Point::new(1.0, 0.0)));
        let state = run(draw_xline, &["v", "3,4"]);
        assert_eq!(
            lines(&state),
            [xline(Point::new(3.0, 4.0), Point::new(0.0, 1.0))]
        );
    }

    #[test]
    fn ang_takes_degrees() {
        let state = run(draw_xline, &["a", "90", "1,1"]);
        let (_, d, _) = lines(&state)[0].infinite_line().unwrap();
        assert!(near(d, Point::new(0.0, 1.0)));
    }

    #[test]
    fn bisect_halves_the_angle() {
        let state = run(draw_xline, &["b", "0,0", "10,0", "0,10"]);
        let (o, d, _) = lines(&state)[0].infinite_line().unwrap();
        assert!(near(o, Point::new(0.0, 0.0)));
        assert!((d.x - d.y).abs() < 1e-9 && d.x > 0.0);
    }

    #[test]
    fn offset_goes_the_distance_to_the_side_picked() {
        let mut state = AppState::new();
        state.perform(od_commands::AddEntity::new(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        }));
        state.start_command("XLINE", draw_xline);
        state.settle_command();
        for input in ["o", "3"] {
            state.cmd_submit(input);
            state.settle_command();
        }
        state.answer_object(Point::new(5.0, 0.0), Some(od_core::EntityId(0)), false);
        state.settle_command();
        state.cmd_submit("2,-9");
        state.settle_command();
        let made = lines(&state).pop().unwrap();
        let (o, d, _) = made.infinite_line().unwrap();
        assert!(near(d, Point::new(1.0, 0.0)));
        assert!((o.y + 3.0).abs() < 1e-9, "{o:?}");
    }

    #[test]
    fn ray_runs_one_way_from_its_start() {
        let state = run(draw_ray, &["1,1", "1,6"]);
        let found = lines(&state);
        assert_eq!(found, [ray(Point::new(1.0, 1.0), Point::new(0.0, 1.0))]);
    }

    #[test]
    fn a_through_point_on_the_base_is_refused() {
        let state = run(draw_xline, &["2,2", "2,2"]);
        assert!(lines(&state).is_empty());
    }
}
