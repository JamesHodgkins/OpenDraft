//! SPLINE: a curve through fit points (the Fit method: cubic, passing
//! through every point, its knots spaced by Knots) or from control vertices
//! (the CV method, of the chosen Degree). The geometry is
//! `od_core::Spline`; this only asks.
//!
//! Method, Knots and Degree are remembered as `SPLMETHOD`, `SPLKNOTS` and
//! `SPLDEGREE` (`spline_memory.rs`). Not built yet, and said so when chosen
//! rather than ignored: Object (converting a polyline), start and end
//! tangency, a fit tolerance other than 0, and Close. As in AutoCAD, Esc
//! discards the spline being drawn.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::{Entity, KnotParameterization, Point, Spline};

use crate::draw::{add, distance, MIN_SIZE};
use crate::spline_memory::{memory, remember, Method, DEGREE_RANGE};

/// Most points one spline may be given.
const MAX_POINTS: usize = 10_000;

fn knots_label(knots: KnotParameterization) -> &'static str {
    match knots {
        KnotParameterization::Chord => "Chord",
        KnotParameterization::SqrtChord => "Square root",
        KnotParameterization::Uniform => "Uniform",
    }
}

/// SPLINE. See the module notes.
pub fn draw_spline(ed: &mut Editor) -> Result<(), Cancelled> {
    // The CV degree for this spline: SPLDEGREE, or what Degree gave.
    let mut degree = memory().degree;
    let first = loop {
        let m = memory();
        let options = PointOptions::new("Specify first point").keyword("Method", "M");
        let options = match m.method {
            Method::Fit => {
                ed.say(format!(
                    "Current settings: Method=Fit   Knots={}",
                    knots_label(m.knots)
                ));
                options.keyword("Knots", "K")
            }
            Method::Cv => {
                ed.say(format!("Current settings: Method=CV   Degree={degree}"));
                options.keyword("Degree", "D")
            }
        };
        match ed.ask_point(options.keyword("Object", "O"))? {
            Answer::Value(first) => break first,
            Answer::Keyword(k) if k == "Method" => ask_method(ed)?,
            Answer::Keyword(k) if k == "Knots" => ask_knots(ed)?,
            Answer::Keyword(k) if k == "Degree" => degree = ask_degree(ed, degree)?,
            Answer::Keyword(_) => ed.say("Converting a polyline to a spline is not built yet."),
            Answer::Enter => return Ok(()),
        }
    };
    match memory().method {
        Method::Fit => by_fit_points(ed, first, memory().knots),
        Method::Cv => by_control_vertices(ed, first, degree),
    }
}

fn ask_method(ed: &Editor) -> Result<(), Cancelled> {
    let current = match memory().method {
        Method::Fit => "Fit",
        Method::Cv => "CV",
    };
    let answer = ed.get_keyword(
        Options::<String>::new("Enter spline creation method")
            .keyword("Fit", "F")
            .keyword("CV", "C")
            .default(current.to_owned()),
    )?;
    remember(|m| {
        m.method = if answer == "CV" {
            Method::Cv
        } else {
            Method::Fit
        }
    });
    Ok(())
}

fn ask_knots(ed: &Editor) -> Result<(), Cancelled> {
    let answer = ed.get_keyword(
        Options::<String>::new("Enter knot parameterization")
            .keyword("Chord", "C")
            .keyword("Square root", "S")
            .keyword("Uniform", "U")
            .default(knots_label(memory().knots).to_owned()),
    )?;
    let knots = match answer.as_str() {
        "Square root" => KnotParameterization::SqrtChord,
        "Uniform" => KnotParameterization::Uniform,
        _ => KnotParameterization::Chord,
    };
    remember(|m| m.knots = knots);
    Ok(())
}

/// A whole degree from 1 to 10; one SPLDEGREE allows (1 to 5) is also
/// remembered.
fn ask_degree(ed: &Editor, current: u8) -> Result<u8, Cancelled> {
    let most = od_core::spline::MAX_COMMAND_DEGREE;
    loop {
        let options = NumberOptions::new("Enter degree of spline").default(f64::from(current));
        let typed = ed.get_number(options)?;
        if typed.fract() == 0.0 && (1.0..=f64::from(most)).contains(&typed) {
            let degree = typed as u8;
            if (DEGREE_RANGE.0..=DEGREE_RANGE.1).contains(&degree) {
                remember(|m| m.degree = degree);
            }
            return Ok(degree);
        }
        ed.say(format!("Requires an integer between 1 and {most}."));
    }
}

/// Only a fit tolerance of 0 (through the points) is built.
fn ask_tolerance(ed: &Editor) -> Result<(), Cancelled> {
    loop {
        let options = NumberOptions::new("Specify fit tolerance").default(0.0);
        if ed.get_distance(options)? == 0.0 {
            return Ok(());
        }
        ed.say("Positive spline fit tolerance is not implemented; enter 0.");
    }
}

/// Adds `at` to `points` unless it repeats the last one or there are too
/// many already.
fn push(ed: &Editor, points: &mut Vec<Point>, at: Point) {
    let last = points.last().copied().unwrap_or(at);
    if points.len() >= MAX_POINTS {
        ed.say(format!("A spline takes at most {MAX_POINTS} points."));
    } else if points.is_empty() || distance(last, at) > MIN_SIZE {
        points.push(at);
    }
}

/// The points so far and `at`, without repeating the last.
fn with_point(points: &[Point], at: Point) -> Vec<Point> {
    let mut all = points.to_vec();
    if all.last().is_none_or(|last| distance(*last, at) > MIN_SIZE) {
        all.push(at);
    }
    all
}

fn polygon(points: Vec<Point>) -> Entity {
    Entity::Polyline {
        points,
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

/// The fit spline through `points`, or the polygon through them when they
/// make none (yet).
fn fit_ghost(points: Vec<Point>, knots: KnotParameterization) -> Vec<Entity> {
    match Spline::from_fit_points(points.clone(), knots) {
        Ok(s) => vec![Entity::Spline(Box::new(s))],
        Err(_) if points.len() >= 2 => vec![polygon(points)],
        Err(_) => Vec::new(),
    }
}

/// The CV spline of `degree` (lowered to what the points allow), or `None`
/// for fewer than two points.
fn cv_spline(points: Vec<Point>, degree: u8) -> Option<Spline> {
    let most = u8::try_from(points.len().checked_sub(1)?).unwrap_or(u8::MAX);
    Spline::from_control_points(degree.min(most).max(1), points).ok()
}

fn by_fit_points(ed: &Editor, first: Point, knots: KnotParameterization) -> Result<(), Cancelled> {
    let mut points = vec![first];
    loop {
        let n = points.len();
        let shown = points.clone();
        let mut options = PointOptions::new("Enter next point")
            .base(points[n - 1])
            .ghost(move |at| fit_ghost(with_point(&shown, at), knots))
            .keyword(
                if n < 2 {
                    "start Tangency"
                } else {
                    "end Tangency"
                },
                "T",
            )
            .keyword("toLerance", "L");
        if n >= 2 {
            options = options.keyword("Undo", "U");
        }
        if n >= 3 {
            options = options.keyword("Close", "C");
        }
        match ed.ask_point(options)? {
            Answer::Value(at) => push(ed, &mut points, at),
            Answer::Keyword(k) if k == "Undo" => {
                points.pop();
            }
            Answer::Keyword(k) if k == "toLerance" => ask_tolerance(ed)?,
            Answer::Keyword(k) if k == "Close" => ed.say("Closed splines are not built yet."),
            Answer::Keyword(_) => ed.say("Spline end tangents are not built yet."),
            Answer::Enter => break,
        }
    }
    if points.len() < 2 {
        return Ok(());
    }
    match Spline::from_fit_points(points, knots) {
        Ok(s) => add(ed, Entity::Spline(Box::new(s)), "Spline"),
        Err(why) => {
            ed.say(format!("Cannot fit a spline through those points: {why}."));
            Ok(())
        }
    }
}

fn by_control_vertices(ed: &Editor, first: Point, degree: u8) -> Result<(), Cancelled> {
    let mut points = vec![first];
    loop {
        let n = points.len();
        let shown = points.clone();
        let mut options = PointOptions::new("Enter next point")
            .base(points[n - 1])
            .ghost(move |at| {
                // The control frame, and the curve once there is one.
                let all = with_point(&shown, at);
                let mut ghost = vec![polygon(all.clone())];
                ghost.extend(cv_spline(all, degree).map(|s| Entity::Spline(Box::new(s))));
                ghost
            });
        if n >= 2 {
            options = options.keyword("Undo", "U");
        }
        if n >= 3 {
            options = options.keyword("Close", "C");
        }
        match ed.ask_point(options)? {
            Answer::Value(at) => push(ed, &mut points, at),
            Answer::Keyword(k) if k == "Undo" => {
                points.pop();
            }
            Answer::Keyword(_) => ed.say("Closed splines are not built yet."),
            Answer::Enter => break,
        }
    }
    let count = points.len();
    let Some(spline) = cv_spline(points, degree) else {
        return Ok(());
    };
    if spline.degree < degree {
        ed.say(format!(
            "Degree lowered to {} to suit {count} control vertices.",
            spline.degree
        ));
    }
    add(ed, Entity::Spline(Box::new(spline)), "Spline")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spline_memory::take_turn_for_tests;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn splines(state: &AppState) -> Vec<Spline> {
        let objects = state.session().document().objects();
        objects
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Spline(s) => Some((**s).clone()),
                _ => None,
            })
            .collect()
    }

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_command("Spline", draw_spline);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn said(state: &AppState, text: &str) -> bool {
        state.cmdline.log.iter().any(|l| l.contains(text))
    }

    #[test]
    fn fit_points_make_a_cubic_through_them_in_one_undo_step() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "3,4", "6,0", "9,4", ""]);
        assert!(!state.command_running());
        assert!(said(&state, "Current settings: Method=Fit   Knots=Chord"));
        let [s] = splines(&state).try_into().unwrap();
        assert_eq!(s.degree, 3);
        let fit = s.fit.clone().unwrap();
        assert_eq!(
            fit.points,
            [p(0.0, 0.0), p(3.0, 4.0), p(6.0, 0.0), p(9.0, 4.0)]
        );
        assert_eq!(fit.parameterization, Some(KnotParameterization::Chord));
        for q in &fit.points {
            assert!(distance(s.nearest(*q).0, *q) < 1e-9);
        }
        state.undo();
        assert!(splines(&state).is_empty(), "one undo step");
    }

    #[test]
    fn too_few_points_or_esc_leave_nothing() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(&mut state, &["0,0", ""]);
        assert!(!state.command_running());
        run(&mut state, &["0,0", "3,4", "6,0"]);
        state.cancel();
        state.settle_command();
        assert!(splines(&state).is_empty());
        assert!(!state.session().history().can_undo());
    }

    #[test]
    fn undo_takes_back_the_last_point() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "5,5", "3,4", "u", "6,0", ""]);
        let fit = splines(&state)[0].fit.clone().unwrap();
        assert_eq!(fit.points, [p(0.0, 0.0), p(5.0, 5.0), p(6.0, 0.0)]);
    }

    #[test]
    fn knots_are_chosen_and_remembered() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(&mut state, &["k", "u", "0,0", "3,4", "6,0", ""]);
        let fit = splines(&state)[0].fit.clone().unwrap();
        assert_eq!(fit.parameterization, Some(KnotParameterization::Uniform));
        assert_eq!(memory().knots, KnotParameterization::Uniform);
    }

    #[test]
    fn options_not_built_yet_say_so_and_change_nothing() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(
            &mut state,
            &["o", "0,0", "t", "l", "1", "0", "3,4", "6,0", "c", ""],
        );
        assert!(said(
            &state,
            "Converting a polyline to a spline is not built yet."
        ));
        assert!(said(&state, "Spline end tangents are not built yet."));
        assert!(said(
            &state,
            "Positive spline fit tolerance is not implemented; enter 0."
        ));
        assert!(said(&state, "Closed splines are not built yet."));
        let [s] = splines(&state).try_into().unwrap();
        assert!(!s.closed);
        assert_eq!(s.fit.unwrap().points.len(), 3);
    }

    #[test]
    fn control_vertices_make_a_spline_of_the_chosen_degree() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        run(
            &mut state,
            &["m", "cv", "0,0", "1,2", "3,-1", "4,2", "6,0", ""],
        );
        assert_eq!(memory().method, Method::Cv, "the method is remembered");
        let s = splines(&state).remove(0);
        assert_eq!(
            (s.degree, s.control_points.len(), s.fit.is_none()),
            (3, 5, true)
        );
        assert_eq!((s.start_point(), s.end_point()), (p(0.0, 0.0), p(6.0, 0.0)));
        // Degree for this spline: 7 is refused only past 10, and is not SPLDEGREE.
        run(&mut state, &["d", "2.5", "11", "7", "0,0", "1,1", ""]);
        assert!(said(&state, "Requires an integer between 1 and 10."));
        assert!(said(
            &state,
            "Degree lowered to 1 to suit 2 control vertices."
        ));
        assert_eq!(splines(&state)[1].degree, 1);
        assert_eq!(memory().degree, 3);
        run(&mut state, &["d", "2", "0,0", "1,1", "2,0", ""]);
        assert_eq!((splines(&state)[2].degree, memory().degree), (2, 2));
    }

    #[test]
    fn the_ghost_shows_the_curve_through_the_cursor() {
        let shown = fit_ghost(
            with_point(&[p(0.0, 0.0), p(3.0, 4.0)], p(6.0, 0.0)),
            KnotParameterization::Chord,
        );
        assert!(matches!(&shown[..], [Entity::Spline(_)]));
        let lone = fit_ghost(
            with_point(&[p(0.0, 0.0)], p(0.0, 0.0)),
            KnotParameterization::Chord,
        );
        assert!(lone.is_empty());
        assert_eq!(cv_spline(vec![p(0.0, 0.0)], 3), None);
        assert_eq!(
            cv_spline(vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 1.0)], 3)
                .unwrap()
                .degree,
            2
        );
    }
}
