//! ELLIPSE: a full ellipse by the two ends of one axis or by its centre, then
//! the distance to the other axis; with Arc, an elliptical arc whose ends are
//! given as angles, parameters or an included angle. The geometry is
//! `od_core::Ellipse`; this only asks.
//!
//! Start and end angles and parameters are measured counter-clockwise from
//! the first axis endpoint given (whichever axis turns out the major one);
//! a picked point gives the direction from the centre (angles) or the
//! nearest point of the ellipse (parameters). Rotation tilts a circle whose
//! diameter is the first axis, as AutoCAD does. Isocircle needs isometric
//! drafting, which does not exist yet.

use std::f64::consts::{PI, TAU};

use od_commands::{Answer, Cancelled, Editor, NumberOptions, NumberOrPoint, PointOptions};
use od_core::{Ellipse, EllipseError, Entity, Point};

use crate::draw::{add, distance, line, MIN_SIZE};

const INCLUDED: &str = "Included angle";

/// The axes as given: the centre, the semi-axis to the first endpoint, and
/// the length of the other semi-axis.
#[derive(Clone, Copy)]
struct Axes {
    center: Point,
    first: Point,
    other: f64,
}

impl Axes {
    /// The full ellipse, in canonical form.
    fn ellipse(self) -> Result<Ellipse, EllipseError> {
        Ellipse::from_axes(self.center, self.first, self.other, 0.0, TAU)
    }

    /// The direction of the first axis endpoint from the centre.
    fn reference(self) -> f64 {
        self.first.y.atan2(self.first.x)
    }

    /// How much a parameter measured from the first axis differs from the
    /// canonical one (a quarter turn when the other axis is the major one).
    fn shift(self) -> f64 {
        if self.other > self.first.x.hypot(self.first.y) {
            PI / 2.0
        } else {
            0.0
        }
    }
}

/// How far `at` is from the line through `center` along `axis`.
fn across(center: Point, axis: Point, at: Point) -> f64 {
    let len = axis.x.hypot(axis.y);
    ((at.x - center.x) * axis.y - (at.y - center.y) * axis.x).abs() / len
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new(a.x + (b.x - a.x) / 2.0, a.y + (b.y - a.y) / 2.0)
}

/// ELLIPSE. See the module notes.
pub fn draw_ellipse(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut arc = false;
    let axes = loop {
        let options = if arc {
            PointOptions::new("Specify axis endpoint of elliptical arc").keyword("Center", "C")
        } else {
            PointOptions::new("Specify axis endpoint of ellipse")
                .keyword("Arc", "A")
                .keyword("Center", "C")
        };
        match ed.ask_point(options)? {
            Answer::Value(first) => break by_axis_ends(ed, first)?,
            Answer::Keyword(k) if k == "Arc" => arc = true,
            Answer::Keyword(_) => break by_center(ed)?,
            Answer::Enter => return Err(Cancelled),
        }
    };
    let whole = match axes.ellipse() {
        Ok(whole) => whole,
        Err(_) => {
            ed.say("The minor-to-major radius ratio must be between 0.000001 and 1.");
            return Ok(());
        }
    };
    let made = if arc {
        arc_ends(ed, axes, whole)?
    } else {
        whole
    };
    add(ed, Entity::Ellipse(made), "Ellipse")
}

/// The first axis from its two ends, then the other axis.
fn by_axis_ends(ed: &Editor, start: Point) -> Result<Axes, Cancelled> {
    let end = loop {
        let options = PointOptions::new("Specify other endpoint of axis")
            .base(start)
            .ghost(move |at| vec![line(start, at)]);
        let end = ed.get_point(options)?;
        if distance(start, end) > MIN_SIZE {
            break end;
        }
        ed.say("Requires a non-zero axis length.");
    };
    let center = midpoint(start, end);
    let first = Point::new(start.x - center.x, start.y - center.y);
    other_axis(ed, center, first, line(start, end))
}

/// The centre and one axis endpoint, then the other axis.
fn by_center(ed: &Editor) -> Result<Axes, Cancelled> {
    let center = ed.get_point("Specify center of ellipse")?;
    let end = loop {
        let options = PointOptions::new("Specify endpoint of axis")
            .base(center)
            .ghost(move |at| vec![line(center, at)]);
        let end = ed.get_point(options)?;
        if distance(center, end) > MIN_SIZE {
            break end;
        }
        ed.say("Requires a non-zero axis length.");
    };
    let first = Point::new(end.x - center.x, end.y - center.y);
    let mirror = Point::new(center.x - first.x, center.y - first.y);
    other_axis(ed, center, first, line(mirror, end))
}

/// The other semi-axis: a length, or a point whose distance from the first
/// axis is that length.
fn other_axis(ed: &Editor, center: Point, first: Point, axis: Entity) -> Result<Axes, Cancelled> {
    loop {
        let shown = axis.clone();
        let options = NumberOptions::new("Specify distance to other axis")
            .base(center)
            .keyword("Rotation", "R")
            .ghost(move |at| {
                let other = across(center, first, at);
                let e = Ellipse::from_axes(center, first, other, 0.0, TAU);
                vec![e.map_or_else(|_| shown.clone(), Entity::Ellipse)]
            });
        let other = match ed.ask_distance_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(n)) => n,
            Answer::Value(NumberOrPoint::Point(at)) => across(center, first, at),
            Answer::Keyword(_) => return rotation(ed, center, first),
            Answer::Enter => return Err(Cancelled),
        };
        if other.is_finite() && other > MIN_SIZE {
            return Ok(Axes {
                center,
                first,
                other,
            });
        }
        ed.say("Requires a positive other-axis distance.");
    }
}

/// The largest tilt AutoCAD's Rotation option takes, in degrees.
const MAX_TILT: f64 = 89.4;

/// A tilt given as a direction from the centre, folded into a quarter turn:
/// how far the direction is from the first axis's line.
fn tilt_of(center: Point, first: Point, at: Point) -> f64 {
    let turn = (at.y - center.y).atan2(at.x - center.x) - first.y.atan2(first.x);
    let folded = turn.rem_euclid(PI);
    folded.min(PI - folded)
}

/// Rotation: the first axis is the diameter of a circle tilted by an angle
/// about it, so the other axis is the first's length times its cosine.
fn rotation(ed: &Editor, center: Point, first: Point) -> Result<Axes, Cancelled> {
    let length = first.x.hypot(first.y);
    loop {
        let options = NumberOptions::new("Specify rotation around major axis")
            .base(center)
            .ghost(move |at| {
                let other = length * tilt_of(center, first, at).cos();
                Ellipse::from_axes(center, first, other, 0.0, TAU)
                    .map_or_else(|_| vec![], |e| vec![Entity::Ellipse(e)])
            });
        let tilt = match ed.ask_angle_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(angle)) => angle,
            Answer::Value(NumberOrPoint::Point(at)) => tilt_of(center, first, at),
            _ => return Err(Cancelled),
        };
        if (0.0..=MAX_TILT.to_radians() + 1e-12).contains(&tilt) {
            return Ok(Axes {
                center,
                first,
                other: length * tilt.cos(),
            });
        }
        ed.say("Requires a rotation from 0 to 89.4 degrees.");
    }
}

/// How an arc end is being given.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Angle,
    Parameter,
}

/// The canonical parameter for an answer: an angle from the first axis or a
/// direction from the centre, or a parameter from the first axis or the
/// nearest point of the ellipse.
fn parameter(axes: Axes, whole: &Ellipse, mode: Mode, answer: NumberOrPoint) -> f64 {
    match (mode, answer) {
        (Mode::Angle, NumberOrPoint::Number(a)) => whole.parameter_at_angle(axes.reference() + a),
        (Mode::Angle, NumberOrPoint::Point(at)) => {
            let c = whole.center;
            whole.parameter_at_angle((at.y - c.y).atan2(at.x - c.x))
        }
        (Mode::Parameter, NumberOrPoint::Number(t)) => t - axes.shift(),
        (Mode::Parameter, NumberOrPoint::Point(at)) => whole.nearest(at).1,
    }
}

/// The arc of `whole` from `start` round to `end`.
fn arc_of(whole: &Ellipse, start: f64, end: f64) -> Result<Ellipse, EllipseError> {
    let end = if end <= start { end + TAU } else { end };
    Ellipse::new(
        whole.center,
        whole.major_axis,
        whole.radius_ratio,
        start,
        end,
    )
}

fn arc_ghost(whole: Ellipse, from: f64, axes: Axes, mode: Mode) -> impl Fn(Point) -> Vec<Entity> {
    move |at| {
        let to = parameter(axes, &whole, mode, NumberOrPoint::Point(at));
        arc_of(&whole, from, to).map_or_else(|_| vec![], |e| vec![Entity::Ellipse(e)])
    }
}

/// The start, then the end (or the included angle) of an elliptical arc.
fn arc_ends(ed: &Editor, axes: Axes, whole: Ellipse) -> Result<Ellipse, Cancelled> {
    let center = whole.center;
    let mut mode = Mode::Angle;
    let start = loop {
        let (message, other) = match mode {
            Mode::Angle => ("Specify start angle", ("Parameter", "P")),
            Mode::Parameter => ("Specify start parameter", ("Angle", "A")),
        };
        let options = NumberOptions::new(message)
            .base(center)
            .keyword(other.0, other.1)
            .ghost(move |_| vec![Entity::Ellipse(whole)]);
        match ed.ask_angle_or_point(options)? {
            Answer::Value(answer) => break parameter(axes, &whole, mode, answer),
            Answer::Keyword(k) if k == "Parameter" => mode = Mode::Parameter,
            Answer::Keyword(_) => mode = Mode::Angle,
            Answer::Enter => return Err(Cancelled),
        }
    };
    loop {
        let (message, other) = match mode {
            Mode::Angle => ("Specify end angle", ("Parameter", "P")),
            Mode::Parameter => ("Specify end parameter", ("Angle", "A")),
        };
        let options = NumberOptions::new(message)
            .base(center)
            .keyword(other.0, other.1)
            .keyword(INCLUDED, "I")
            .ghost(arc_ghost(whole, start, axes, mode));
        let made = match ed.ask_angle_or_point(options)? {
            Answer::Value(answer) => arc_of(&whole, start, parameter(axes, &whole, mode, answer)),
            Answer::Keyword(k) if k == "Parameter" => {
                mode = Mode::Parameter;
                continue;
            }
            Answer::Keyword(k) if k == INCLUDED => {
                let options = NumberOptions::new("Specify included angle for arc").default(PI);
                included(&whole, start, ed.get_angle(options)?)
            }
            Answer::Keyword(_) => {
                mode = Mode::Angle;
                continue;
            }
            Answer::Enter => return Err(Cancelled),
        };
        match made {
            Ok(e) => return Ok(e),
            Err(_) => ed.say("Those values make no elliptical arc."),
        }
    }
}

/// The arc from `start` that turns through `angle` as seen from the centre:
/// counter-clockwise when positive, clockwise (so it ends at the start) when
/// negative. A whole turn or more is the full ellipse; no turn is no arc.
fn included(whole: &Ellipse, start: f64, angle: f64) -> Result<Ellipse, EllipseError> {
    if angle.abs() >= TAU {
        return Ok(*whole);
    }
    if angle.abs() < od_core::ellipse::MIN_SWEEP {
        return Err(EllipseError::EmptySweep);
    }
    let end = whole.parameter_at_angle(whole.angle_at(start) + angle);
    if angle >= 0.0 {
        arc_of(whole, start, end)
    } else {
        arc_of(whole, end, start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    fn ellipses(state: &AppState) -> Vec<Ellipse> {
        let objects = state.session().document().objects();
        objects
            .iter()
            .filter_map(|o| match o.entity {
                Entity::Ellipse(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_command("Ellipse", draw_ellipse);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn two_axis_ends_and_a_distance_make_a_full_ellipse() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "10,0", "2"]);
        assert!(!state.command_running(), "one ellipse per command");
        let e = ellipses(&state)[0];
        assert!(e.is_full());
        assert!(near(e.center, p(5.0, 0.0)));
        assert!((e.major_radius() - 5.0).abs() < 1e-12 && (e.minor_radius() - 2.0).abs() < 1e-12);
        state.undo();
        assert!(ellipses(&state).is_empty(), "one undo step");
    }

    #[test]
    fn the_first_axis_may_be_the_minor_one() {
        let mut state = AppState::new();
        run(&mut state, &["c", "1,1", "1,3", "6"]);
        let e = ellipses(&state)[0];
        assert!(near(e.center, p(1.0, 1.0)));
        assert!((e.major_radius() - 6.0).abs() < 1e-12 && (e.minor_radius() - 2.0).abs() < 1e-12);
        assert!(
            e.rotation().abs() < 1e-12 || (e.rotation().abs() - PI).abs() < 1e-12,
            "major along x"
        );
    }

    #[test]
    fn a_clicked_distance_is_measured_across_the_first_axis() {
        let mut state = AppState::new();
        state.start_command("Ellipse", draw_ellipse);
        state.settle_command();
        for at in [p(0.0, 0.0), p(8.0, 0.0), p(6.0, 3.0)] {
            state.answer_point(at);
            state.settle_command();
        }
        let e = ellipses(&state)[0];
        assert!(
            (e.minor_radius() - 3.0).abs() < 1e-12,
            "the height above the axis"
        );
    }

    #[test]
    fn an_arc_by_start_and_end_angles_from_the_first_endpoint() {
        let mut state = AppState::new();
        // First endpoint (0,0), so angle 0 points back at it: the arc runs
        // from (0,0) a quarter of the way round (seen from the centre) to
        // the bottom of the ellipse.
        run(&mut state, &["a", "0,0", "10,0", "2", "0", "90"]);
        let e = ellipses(&state)[0];
        assert!(!e.is_full());
        assert!(near(e.start_point(), p(0.0, 0.0)), "{:?}", e.start_point());
        assert!(near(e.end_point(), p(5.0, -2.0)), "{:?}", e.end_point());
    }

    #[test]
    fn parameters_and_included_angles_can_end_the_arc() {
        let mut state = AppState::new();
        run(&mut state, &["a", "c", "0,0", "4,0", "1", "p", "0", "180"]);
        let e = ellipses(&state)[0];
        assert!(near(e.start_point(), p(4.0, 0.0)) && near(e.end_point(), p(-4.0, 0.0)));
        run(&mut state, &["a", "c", "0,0", "4,0", "1", "0", "i", "-90"]);
        let e = ellipses(&state)[1];
        // Clockwise from the major end: the arc ends there, starting below.
        assert!(near(e.end_point(), p(4.0, 0.0)) && near(e.start_point(), p(0.0, -1.0)));
    }

    #[test]
    fn bad_values_are_refused_and_asked_again() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "0,0"]);
        assert!(state.command_running());
        run(&mut state, &["0,0", "10,0", "0"]);
        assert!(state.command_running());
        assert!(ellipses(&state).is_empty());
        let said =
            |state: &AppState, text: &str| state.cmdline.log.iter().any(|l| l.contains(text));
        assert!(said(&state, "non-zero axis length"));
        assert!(said(&state, "positive other-axis distance"));
        state.cancel();
        run(&mut state, &["a", "0,0", "10,0", "2", "30", "i", "0"]);
        assert!(state.command_running() && said(&state, "no elliptical arc"));
        state.cancel();
        assert!(ellipses(&state).is_empty(), "Esc adds nothing");
    }

    #[test]
    fn rotation_tilts_a_circle_on_the_first_axis() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "10,0", "r", "60"]);
        let e = ellipses(&state)[0];
        assert!((e.major_radius() - 5.0).abs() < 1e-12);
        assert!((e.radius_ratio - 0.5).abs() < 1e-12, "cos 60 = 0.5");
        run(&mut state, &["0,0", "10,0", "r", "0"]);
        assert!(
            (ellipses(&state)[1].radius_ratio - 1.0).abs() < 1e-12,
            "no tilt: round"
        );
        run(&mut state, &["0,0", "10,0", "r", "89.5"]);
        assert!(state.command_running(), "past 89.4 degrees is asked again");
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("from 0 to 89.4")));
        state.cancel();
        assert_eq!(ellipses(&state).len(), 2);
    }

    #[test]
    fn the_first_prompt_offers_arc_and_center() {
        let mut state = AppState::new();
        run(&mut state, &[]);
        let prompt = state.cmd_prompt();
        assert!(prompt.text.starts_with("Specify axis endpoint of ellipse"));
        let labels: Vec<_> = prompt.options.iter().map(|o| o.label.to_string()).collect();
        assert_eq!(labels, ["Arc", "Center"]);
        state.cmd_submit("a");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify axis endpoint of elliptical arc"));
    }
}
