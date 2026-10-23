//! ARC: three points, or any of AutoCAD's start / centre / end combinations
//! with an included angle, chord length, radius or start direction.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::arc::{angle_of, through_points};
use od_core::arc_make::{
    from_center_start_angle, from_center_start_chord, from_center_start_end, from_start_end_angle,
    from_start_end_direction, from_start_end_radius,
};
use od_core::{Entity, Point};

use crate::draw::{add, distance, line, MIN_SIZE};

/// The arc from `a` through `b` to `c`.
pub(crate) fn arc_through(a: Point, b: Point, c: Point) -> Option<Entity> {
    through_points(a, b, c).map(|(center, radius, start, end)| Entity::Arc {
        center,
        radius,
        start,
        end,
    })
}

/// A preview that is a line while the parts make no arc yet.
pub(crate) fn shown(arc: Option<Entity>, from: Point, to: Point) -> Vec<Entity> {
    vec![arc.unwrap_or_else(|| line(from, to))]
}

/// `arc`, or a message and `None` so the caller asks again.
fn or_say(ed: &Editor, arc: Option<Entity>) -> Option<Entity> {
    if arc.is_none() {
        ed.say("Those values make no arc.");
    }
    arc
}

/// ARC. Start point first, then a second point on the arc, or its centre or
/// end; or the centre first.
pub fn draw_arc(ed: &mut Editor) -> Result<(), Cancelled> {
    let options = PointOptions::new("Specify start point of arc").keyword("Center", "C");
    let entity = match ed.ask_point(options)? {
        Answer::Value(start) => from_start(ed, start)?,
        Answer::Keyword(_) => from_centre(ed)?,
        Answer::Enter => return Err(Cancelled),
    };
    add(ed, entity, "Arc")
}

/// ARC, Start, Center, End: the start point, the centre, then the end (or an
/// included angle or chord length).
pub fn draw_arc_sce(ed: &mut Editor) -> Result<(), Cancelled> {
    let start = ed.get_point("Specify start point of arc")?;
    let center = centre_after(ed, start)?;
    let entity = to_end_from_centre(ed, center, start)?;
    add(ed, entity, "Arc")
}

fn from_start(ed: &Editor, start: Point) -> Result<Entity, Cancelled> {
    let options = PointOptions::new("Specify second point of arc")
        .base(start)
        .keyword("Center", "C")
        .keyword("End", "E")
        .ghost(move |at| vec![line(start, at)]);
    match ed.ask_point(options)? {
        Answer::Value(second) => loop {
            let options = PointOptions::new("Specify end point of arc")
                .base(second)
                .ghost(move |at| shown(arc_through(start, second, at), start, at));
            let end = ed.get_point(options)?;
            match arc_through(start, second, end) {
                Some(arc) => return Ok(arc),
                None => ed.say("The three points are in a line."),
            }
        },
        Answer::Keyword(k) if k == "Center" => {
            let center = centre_after(ed, start)?;
            to_end_from_centre(ed, center, start)
        }
        Answer::Keyword(_) => {
            let end = loop {
                let options = PointOptions::new("Specify end point of arc")
                    .base(start)
                    .ghost(move |at| vec![line(start, at)]);
                let end = ed.get_point(options)?;
                if distance(start, end) > MIN_SIZE {
                    break end;
                }
                ed.say("Requires a non-zero value.");
            };
            to_centre_angle_direction_radius(ed, start, end)
        }
        Answer::Enter => Err(Cancelled),
    }
}

/// A centre point that is not on `start`.
pub(crate) fn centre_after(ed: &Editor, start: Point) -> Result<Point, Cancelled> {
    loop {
        let options = PointOptions::new("Specify center point of arc")
            .base(start)
            .ghost(move |at| vec![line(at, start)]);
        let center = ed.get_point(options)?;
        if distance(center, start) > MIN_SIZE {
            return Ok(center);
        }
        ed.say("Requires a non-zero value.");
    }
}

/// Start and end known: the centre, an included angle, the direction at the
/// start, or a radius finishes the arc.
fn to_centre_angle_direction_radius(
    ed: &Editor,
    start: Point,
    end: Point,
) -> Result<Entity, Cancelled> {
    loop {
        let options = PointOptions::new("Specify center point of arc")
            .base(end)
            .keyword("Angle", "A")
            .keyword("Direction", "D")
            .keyword("Radius", "R")
            .ghost(move |at| shown(from_center_start_end(at, start, end), start, end));
        let arc = match ed.ask_point(options)? {
            Answer::Value(center) => from_center_start_end(center, start, end),
            Answer::Keyword(k) if k == "Angle" => {
                let angle = ed.get_angle(NumberOptions::new("Specify included angle"))?;
                from_start_end_angle(start, end, angle)
            }
            Answer::Keyword(k) if k == "Direction" => {
                let options =
                    NumberOptions::new("Specify tangent direction for the start point of arc")
                        .base(start)
                        .ghost(move |at| {
                            let toward = angle_of(start, at);
                            shown(from_start_end_direction(start, end, toward), start, end)
                        });
                from_start_end_direction(start, end, ed.get_angle(options)?)
            }
            Answer::Keyword(_) => {
                let options = NumberOptions::new("Specify radius of arc").base(start);
                from_start_end_radius(start, end, ed.get_distance(options)?)
            }
            Answer::Enter => return Err(Cancelled),
        };
        if let Some(arc) = or_say(ed, arc) {
            return Ok(arc);
        }
    }
}

/// Centre and start known: the end point, an included angle or a chord
/// length finishes the arc.
pub(crate) fn to_end_from_centre(
    ed: &Editor,
    center: Point,
    start: Point,
) -> Result<Entity, Cancelled> {
    loop {
        let options = PointOptions::new("Specify end point of arc")
            .base(center)
            .keyword("Angle", "A")
            .keyword("chord Length", "L")
            .ghost(move |at| shown(from_center_start_end(center, start, at), center, at));
        let arc = match ed.ask_point(options)? {
            Answer::Value(end) => from_center_start_end(center, start, end),
            Answer::Keyword(k) if k == "Angle" => {
                let angle = ed.get_angle(NumberOptions::new("Specify included angle"))?;
                from_center_start_angle(center, start, angle)
            }
            Answer::Keyword(_) => {
                let options = NumberOptions::new("Specify length of chord").base(start);
                from_center_start_chord(center, start, ed.get_distance(options)?)
            }
            Answer::Enter => return Err(Cancelled),
        };
        if let Some(arc) = or_say(ed, arc) {
            return Ok(arc);
        }
    }
}

fn from_centre(ed: &Editor) -> Result<Entity, Cancelled> {
    let center = ed.get_point("Specify center point of arc")?;
    let start = loop {
        let options = PointOptions::new("Specify start point of arc")
            .base(center)
            .ghost(move |at| vec![line(center, at)]);
        let start = ed.get_point(options)?;
        if distance(center, start) > MIN_SIZE {
            break start;
        }
        ed.say("Requires a non-zero value.");
    };
    to_end_from_centre(ed, center, start)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_command("Test", draw_arc);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn start_center_end_goes_counter_clockwise_from_the_start() {
        let mut state = AppState::new();
        state.start_command("Arc", draw_arc_sce);
        state.settle_command();
        for input in ["5,0", "0,0", "0,5"] {
            state.cmd_submit(input);
            state.settle_command();
        }
        assert!(!state.command_running());
        let found = entities(&state);
        let (center, radius, _, _) = arc_of(&found[0]);
        assert_eq!((center, radius), (p(0.0, 0.0), 5.0));
        assert!((sweep(&found[0]) - FRAC_PI_2).abs() < 1e-9);
    }

    fn arc_of(e: &Entity) -> (Point, f64, f64, f64) {
        let Entity::Arc {
            center,
            radius,
            start,
            end,
        } = e
        else {
            panic!("expected an arc, got {e:?}")
        };
        (*center, *radius, *start, *end)
    }

    fn sweep(e: &Entity) -> f64 {
        let (_, _, start, end) = arc_of(e);
        od_core::arc::sweep(start, end)
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-9
    }

    fn labels(state: &mut AppState) -> Vec<String> {
        let options = state.cmd_prompt().options;
        options.iter().map(|o| o.label.to_string()).collect()
    }

    #[test]
    fn arc_through_three_points_runs_from_the_first_to_the_last() {
        let mut state = AppState::new();
        run(&mut state, &["1,0", "0,1", "-1,0"]);
        assert!(!state.command_running());
        let (center, radius, start, end) = arc_of(&entities(&state)[0]);
        assert!(near(center, p(0.0, 0.0)) && (radius - 1.0).abs() < 1e-9);
        assert!(start.abs() < 1e-9 && (end - PI).abs() < 1e-9);
        // The same points the other way round: drawn clockwise, kept counter-clockwise.
        run(&mut state, &["-1,0", "0,1", "1,0"]);
        let (_, _, start, end) = arc_of(&entities(&state)[1]);
        assert!(start.abs() < 1e-9 && (end - PI).abs() < 1e-9, "same arc");
    }

    #[test]
    fn the_prompts_offer_the_options_of_each_stage() {
        let mut state = AppState::new();
        run(&mut state, &[]);
        assert_eq!(labels(&mut state), ["Center"]);
        run_more(&mut state, &["5,0"]);
        assert_eq!(labels(&mut state), ["Center", "End"]);
        run_more(&mut state, &["e", "0,5"]);
        assert_eq!(labels(&mut state), ["Angle", "Direction", "Radius"]);
        state.cancel();
        run(&mut state, &["5,0", "c", "0,0"]);
        assert_eq!(labels(&mut state), ["Angle", "chord Length"]);
    }

    fn run_more(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn start_centre_end_runs_counter_clockwise_to_the_end_direction() {
        let mut state = AppState::new();
        run(&mut state, &["5,0", "c", "0,0", "0,9"]);
        let (center, radius, start, end) = arc_of(&entities(&state)[0]);
        assert_eq!(center, p(0.0, 0.0));
        assert!((radius - 5.0).abs() < 1e-9, "radius from the start point");
        assert!(start.abs() < 1e-9 && (end - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn start_centre_angle_uses_the_included_angle() {
        let mut state = AppState::new();
        run(&mut state, &["5,0", "c", "0,0", "a", "90"]);
        assert!((sweep(&entities(&state)[0]) - FRAC_PI_2).abs() < 1e-9);
        run(&mut state, &["5,0", "c", "0,0", "a", "-90"]);
        let clockwise = &entities(&state)[1];
        let (c, r, _, end) = arc_of(clockwise);
        assert!((sweep(clockwise) - FRAC_PI_2).abs() < 1e-9);
        assert!(near(od_core::arc::point_at(c, r, end), p(5.0, 0.0)));
    }

    #[test]
    fn start_centre_chord_length_gives_the_short_arc() {
        let mut state = AppState::new();
        run(&mut state, &["5,0", "c", "0,0", "l", "7.0710678118654755"]);
        assert!((sweep(&entities(&state)[0]) - FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn start_end_radius_gives_the_short_arc_for_a_positive_radius() {
        let mut state = AppState::new();
        run(&mut state, &["5,0", "e", "0,5", "r", "5"]);
        let arc = &entities(&state)[0];
        let (c, r, ..) = arc_of(arc);
        assert!(near(c, p(0.0, 0.0)) && (r - 5.0).abs() < 1e-9);
        assert!((sweep(arc) - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn start_end_angle_direction_and_centre_all_finish_the_arc() {
        let mut state = AppState::new();
        run(&mut state, &["5,0", "e", "0,5", "a", "90"]);
        assert!(near(arc_of(&entities(&state)[0]).0, p(0.0, 0.0)));
        run(&mut state, &["5,0", "e", "0,5", "d", "90"]);
        assert!(near(arc_of(&entities(&state)[1]).0, p(0.0, 0.0)));
        run(&mut state, &["5,0", "e", "0,5", "0,0"]);
        assert!(near(arc_of(&entities(&state)[2]).0, p(0.0, 0.0)));
        assert!(!state.command_running());
    }

    #[test]
    fn centre_first_takes_the_centre_then_the_start_then_an_end_or_angle() {
        let mut state = AppState::new();
        run(&mut state, &["c", "0,0", "5,0", "0,9"]);
        let (c, r, start, end) = arc_of(&entities(&state)[0]);
        assert!(near(c, p(0.0, 0.0)) && (r - 5.0).abs() < 1e-9);
        assert!(start.abs() < 1e-9 && (end - FRAC_PI_2).abs() < 1e-9);
        run(&mut state, &["c", "0,0", "5,0", "a", "180"]);
        assert!((sweep(&entities(&state)[1]) - PI).abs() < 1e-9);
    }

    #[test]
    fn values_that_make_no_arc_are_refused_and_asked_again() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "e", "10,0", "r", "2"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
        assert!(state.cmdline.log.iter().any(|l| l.contains("no arc")));
        state.cmd_submit("0,5");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn arc_points_in_a_line_are_refused_and_asked_again() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "1,1", "2,2"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
        assert!(state.cmdline.log.iter().any(|l| l.contains("in a line")));
        state.cmd_submit("2,0");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
    }
}
