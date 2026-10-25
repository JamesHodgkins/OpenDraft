//! DIST and AREA: measure and say. Neither changes the drawing. The maths is
//! `od_core::measure`; these commands only ask and report, in the drawing's
//! units, on the command line.

use od_commands::{Answer, Cancelled, DynamicDimensions, Editor, Options, Picked, PointOptions};
use od_core::measure::{corners, separation, Measured};
use od_core::units::{format_angle, format_area, format_length};
use od_core::{Entity, Point, Units};

use crate::draw::line;

fn drawing_units(ed: &Editor) -> Result<Units, Cancelled> {
    ed.read(|doc| *doc.units())
}

/// `Distance = 5.0000, Angle in XY Plane = 53` and the offsets under it.
fn say_separation(ed: &Editor, units: &Units, a: Point, b: Point) {
    let s = separation(a, b);
    ed.say(format!(
        "Distance = {}, Angle in XY Plane = {}",
        format_length(units, s.distance),
        format_angle(units, s.angle)
    ));
    ed.say(format!(
        "Delta X = {}, Delta Y = {}",
        format_length(units, s.dx),
        format_length(units, s.dy)
    ));
}

/// DIST: the distance and angle between two points. Multiple points adds up
/// the length of a path of points.
pub fn measure_distance(ed: &mut Editor) -> Result<(), Cancelled> {
    let first = ed.get_point("Specify first point")?;
    let options = PointOptions::new("Specify second point")
        .keyword("Multiple points", "M")
        .dimensions(DynamicDimensions::Polar)
        .base(first)
        .ghost(move |at| vec![line(first, at)]);
    match ed.ask_point(options)? {
        Answer::Value(second) => {
            let units = drawing_units(ed)?;
            say_separation(ed, &units, first, second);
            Ok(())
        }
        Answer::Keyword(_) => measure_path(ed, first),
        Answer::Enter => Ok(()),
    }
}

/// The running total along a path of points; Enter says it and ends, Undo
/// takes back the last point.
fn measure_path(ed: &Editor, first: Point) -> Result<(), Cancelled> {
    let units = drawing_units(ed)?;
    // The points so far and the length of each step to one.
    let mut points = vec![first];
    let mut steps: Vec<f64> = Vec::new();
    loop {
        let from = points.last().copied().unwrap_or(first);
        let mut options = PointOptions::new("Specify next point")
            .dimensions(DynamicDimensions::Polar)
            .base(from)
            .ghost(move |at| vec![line(from, at)]);
        if !steps.is_empty() {
            options = options.keyword("Undo", "U");
        }
        match ed.ask_point(options) {
            Err(Cancelled) | Ok(Answer::Enter) => {
                let total: f64 = steps.iter().sum();
                ed.say(format!("Total distance = {}", format_length(&units, total)));
                return Ok(());
            }
            Ok(Answer::Value(to)) => {
                let step = separation(from, to).distance;
                steps.push(step);
                points.push(to);
                let total: f64 = steps.iter().sum();
                ed.say(format!(
                    "Distance = {}, total distance = {}",
                    format_length(&units, step),
                    format_length(&units, total)
                ));
            }
            Ok(Answer::Keyword(_)) => {
                steps.pop();
                if points.len() > 1 {
                    points.pop();
                }
            }
        }
    }
}

/// Whether AREA is measuring one shape, or adding to or taking from a total.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Single,
    Add,
    Subtract,
}

/// AREA: the area and perimeter of a shape given by corner points or of an
/// object. Add area and Subtract area keep a running total across shapes.
pub fn measure_area(ed: &mut Editor) -> Result<(), Cancelled> {
    let units = drawing_units(ed)?;
    let mut mode = Mode::Single;
    let mut total = 0.0;
    loop {
        let options = PointOptions::new("Specify first corner point").keyword("Object", "O");
        let options = match mode {
            Mode::Single => options
                .keyword("Add area", "A")
                .keyword("Subtract area", "S"),
            Mode::Add => options.keyword("Subtract area", "S").keyword("eXit", "X"),
            Mode::Subtract => options.keyword("Add area", "A").keyword("eXit", "X"),
        };
        let shape = match ed.ask_point(options) {
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            Ok(Answer::Value(first)) => corner_shape(ed, &units, first)?,
            Ok(Answer::Keyword(k)) => match k.as_str() {
                "Add area" => {
                    mode = Mode::Add;
                    continue;
                }
                "Subtract area" => {
                    mode = Mode::Subtract;
                    continue;
                }
                "eXit" => return Ok(()),
                _ => object_shape(ed, &units)?,
            },
        };
        let Some(area) = shape else {
            continue;
        };
        match mode {
            Mode::Single => return Ok(()),
            Mode::Add => total += area,
            Mode::Subtract => total -= area,
        }
        ed.say(format!("Total area = {}", format_area(&units, total)));
    }
}

/// Ask for corner points until Enter, then say the area and perimeter.
/// `None` when there were too few corners to enclose anything.
fn corner_shape(ed: &Editor, units: &Units, first: Point) -> Result<Option<f64>, Cancelled> {
    let mut points = vec![first];
    loop {
        let so_far = points.clone();
        let from = points.last().copied().unwrap_or(first);
        let options = PointOptions::new("Specify next corner point or press ENTER for total")
            .base(from)
            .ghost(move |at| {
                let mut all = so_far.clone();
                all.push(at);
                vec![Entity::Polyline {
                    points: all,
                    closed: true,
                    bulges: Vec::new(),
                    widths: Vec::new(),
                }]
            });
        match ed.ask_point(options) {
            Err(Cancelled) | Ok(Answer::Enter) => break,
            Ok(Answer::Value(next)) => points.push(next),
            Ok(Answer::Keyword(_)) => {}
        }
    }
    if points.len() < 3 {
        ed.say("Give at least three corner points.");
        return Ok(None);
    }
    let (area, perimeter) = corners(&points);
    ed.say(format!(
        "Area = {}, Perimeter = {}",
        format_area(units, area),
        format_length(units, perimeter)
    ));
    Ok(Some(area))
}

/// Ask for an object and say its area and perimeter (or length).
fn object_shape(ed: &Editor, units: &Units) -> Result<Option<f64>, Cancelled> {
    let picked = match ed.ask_object(Options::<Picked>::new("Select object"))? {
        Answer::Value(picked) => picked,
        _ => return Ok(None),
    };
    let id = picked.id;
    let measured = ed.read(move |doc| doc.get(id).and_then(Entity::measured))?;
    let Some(m) = measured else {
        ed.say("That object has no area.");
        return Ok(None);
    };
    Ok(say_object(ed, units, &m))
}

/// Report an object; an open one gives a length and the area it would
/// enclose if closed, and a line or arc only a length.
fn say_object(ed: &Editor, units: &Units, m: &Measured) -> Option<f64> {
    let length = format_length(units, m.length);
    let name = if m.closed { "Perimeter" } else { "Length" };
    match m.area {
        Some(area) => {
            ed.say(format!(
                "Area = {}, {name} = {length}",
                format_area(units, area)
            ));
            Some(area)
        }
        None => {
            ed.say(format!("{name} = {length}. It encloses no area."));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn said(state: &AppState, text: &str) -> bool {
        state.cmdline.log.iter().any(|l| l.contains(text))
    }

    fn start(command: fn(&mut Editor) -> Result<(), Cancelled>) -> AppState {
        let mut state = AppState::new();
        state.start_command("Test", command);
        state.settle_command();
        state
    }

    fn points(state: &mut AppState, at: &[Point]) {
        for &point in at {
            state.answer_point(point);
            state.settle_command();
        }
    }

    #[test]
    fn distance_reports_length_angle_and_offsets() {
        let mut state = start(measure_distance);
        points(&mut state, &[p(1.0, 1.0), p(4.0, 5.0)]);
        assert!(said(&state, "Distance = 5.0000, Angle in XY Plane = 53"));
        assert!(said(&state, "Delta X = 3.0000, Delta Y = 4.0000"));
        assert!(!state.command_running());
        assert!(state.session().document().objects().is_empty());
    }

    #[test]
    fn multiple_points_keep_a_running_total() {
        let mut state = start(measure_distance);
        points(&mut state, &[p(0.0, 0.0)]);
        state.answer_keyword("Multiple points");
        state.settle_command();
        points(&mut state, &[p(3.0, 0.0), p(3.0, 4.0)]);
        assert!(said(&state, "total distance = 7.0000"));
        state.answer_enter();
        state.settle_command();
        assert!(said(&state, "Total distance = 7.0000"));
        assert!(!state.command_running());
    }

    #[test]
    fn area_of_corner_points() {
        let mut state = start(measure_area);
        points(&mut state, &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]);
        state.answer_enter();
        state.settle_command();
        assert!(said(&state, "Area = 6.0000, Perimeter = 12.0000"));
        assert!(!state.command_running());
    }

    #[test]
    fn two_corners_enclose_nothing() {
        let mut state = start(measure_area);
        points(&mut state, &[p(0.0, 0.0), p(4.0, 0.0)]);
        state.answer_enter();
        state.settle_command();
        assert!(said(&state, "at least three corner points"));
    }

    #[test]
    fn area_of_an_object_and_a_running_total() {
        let mut state = AppState::new();
        state.add_entity(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        });
        state.add_entity(Entity::Rectangle {
            min: p(10.0, 10.0),
            max: p(12.0, 11.0),
        });
        state.start_command("Test", measure_area);
        state.settle_command();
        state.answer_keyword("Add area");
        state.settle_command();
        state.answer_keyword("Object");
        state.settle_command();
        let ids: Vec<_> = state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        state.answer_object(p(1.0, 0.0), Some(ids[0]), false);
        state.settle_command();
        assert!(said(&state, "Area = 3.1416, Perimeter = 6.2832"));
        state.answer_keyword("Subtract area");
        state.settle_command();
        state.answer_keyword("Object");
        state.settle_command();
        state.answer_object(p(10.0, 10.5), Some(ids[1]), false);
        state.settle_command();
        assert!(said(&state, "Area = 2.0000, Perimeter = 6.0000"));
        assert!(said(&state, "Total area = 1.1416"));
        assert!(state.command_running(), "still adding and subtracting");
    }

    #[test]
    fn a_line_has_a_length_but_no_area() {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: p(0.0, 0.0),
            end: p(3.0, 4.0),
        });
        state.start_command("Test", measure_area);
        state.settle_command();
        state.answer_keyword("Object");
        state.settle_command();
        let id = state.session().document().objects()[0].id;
        state.answer_object(p(1.5, 2.0), Some(id), false);
        state.settle_command();
        assert!(said(&state, "Length = 5.0000. It encloses no area."));
    }
}
