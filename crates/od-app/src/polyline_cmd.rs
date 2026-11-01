//! PLINE: a chain of line and arc segments, each with a start and end width,
//! that becomes one polyline when Enter or Esc ends it.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::{bulge, width, Entity, Point};

use crate::draw::{add, distance, MIN_SIZE};
use crate::polyline_arc::arc_option;

/// The polyline drawn so far. Like the entity it becomes, it keeps one bulge
/// and one width pair per point: those of the segment from that point to the
/// next, the last pair staying unused until the polyline closes.
#[derive(Clone)]
struct Chain {
    points: Vec<Point>,
    bulges: Vec<f64>,
    widths: Vec<(f64, f64)>,
}

impl Chain {
    fn new(first: Point) -> Self {
        Self {
            points: vec![first],
            bulges: vec![0.0],
            widths: vec![(0.0, 0.0)],
        }
    }

    fn last(&self) -> Point {
        self.points.last().copied().unwrap_or_default()
    }

    /// The direction the next arc leaves in: along the way the last segment
    /// ended (so arcs run on smoothly), or east for the first.
    fn heading(&self) -> Point {
        let n = self.points.len();
        if n < 2 {
            return Point::new(1.0, 0.0);
        }
        bulge::direction_at_end(
            self.points[n - 2],
            self.points[n - 1],
            bulge::at(&self.bulges, n - 2),
        )
    }

    /// Finish the open segment (its bulge and widths), then start the next
    /// one at `to`.
    fn push(&mut self, to: Point, curve: f64, widths: (f64, f64)) {
        self.finish(curve, widths);
        self.points.push(to);
        self.bulges.push(0.0);
        self.widths.push((0.0, 0.0));
    }

    fn finish(&mut self, curve: f64, widths: (f64, f64)) {
        if let Some(last) = self.bulges.last_mut() {
            *last = curve;
        }
        if let Some(last) = self.widths.last_mut() {
            *last = widths;
        }
    }

    /// Take back the last segment.
    fn undo(&mut self) {
        if self.points.len() > 1 {
            self.points.pop();
            self.bulges.pop();
            self.widths.pop();
            self.finish(0.0, (0.0, 0.0));
        }
    }

    fn entity(&self, closed: bool) -> Entity {
        let (mut bulges, mut widths) = (self.bulges.clone(), self.widths.clone());
        if !closed {
            self.unused_last(&mut bulges, &mut widths);
        }
        Entity::Polyline {
            points: self.points.clone(),
            closed,
            bulges: if bulge::has_arcs(&bulges) {
                bulges
            } else {
                Vec::new()
            },
            widths: if width::has_widths(&widths) {
                widths
            } else {
                Vec::new()
            },
        }
    }

    /// The last point of an open polyline starts no segment.
    fn unused_last(&self, bulges: &mut [f64], widths: &mut [(f64, f64)]) {
        if let Some(last) = bulges.last_mut() {
            *last = 0.0;
        }
        if let Some(last) = widths.last_mut() {
            *last = (0.0, 0.0);
        }
    }

    /// What has been drawn so far, for the pointer to snap to: the
    /// segments, as a polyline once there is one.
    fn drawn(&self) -> Vec<Entity> {
        if self.points.len() < 2 {
            return Vec::new();
        }
        let mut shown = self.clone();
        shown.finish(0.0, (0.0, 0.0));
        vec![shown.entity(false)]
    }

    /// How the polyline would look with a segment to `to` added.
    fn with_segment(&self, to: Point, curve: f64, widths: (f64, f64)) -> Entity {
        let mut shown = self.clone();
        shown.push(to, curve, widths);
        shown.entity(false)
    }
}

/// What the drawing mode carries from one prompt to the next.
struct Mode {
    arcs: bool,
    /// Start and end width of the segments drawn from now on.
    widths: (f64, f64),
}

enum Flow {
    Next,
    Finish,
    Close,
}

/// POLYLINE: a chain of points that becomes one object when Enter or Esc
/// ends it. The Arc option makes the next segments arcs that leave smoothly
/// from the one before (Line goes back to straight ones), and offers the other
/// ways to give an arc. Width and Halfwidth set how wide the following
/// segments are, Length continues the last direction by a distance, Undo takes
/// back the last segment and Close joins the end to the start. Fewer than two
/// points draws nothing.
pub fn draw_polyline(ed: &mut Editor) -> Result<(), Cancelled> {
    let first = ed.get_point("Specify start point")?;
    let mut chain = Chain::new(first);
    let mut mode = Mode {
        arcs: false,
        widths: (0.0, 0.0),
    };
    let mut closed = false;
    loop {
        // Esc keeps what was drawn, as LINE does.
        match step(ed, &mut chain, &mut mode) {
            Ok(Flow::Next) => {}
            Ok(Flow::Finish) | Err(Cancelled) => break,
            Ok(Flow::Close) => {
                closed = true;
                break;
            }
        }
    }
    if chain.points.len() < 2 {
        return Ok(());
    }
    add(ed, chain.entity(closed), "Polyline")
}

fn keywords(options: PointOptions, arcs: bool, points: usize) -> PointOptions {
    let names: &[(&str, &str)] = if arcs {
        &[
            ("Angle", "A"),
            ("CEnter", "CE"),
            ("CLose", "CL"),
            ("Direction", "D"),
            ("Halfwidth", "H"),
            ("Line", "L"),
            ("Radius", "R"),
            ("Second pt", "S"),
            ("Undo", "U"),
            ("Width", "W"),
        ]
    } else {
        &[
            ("Arc", "A"),
            ("Close", "C"),
            ("Halfwidth", "H"),
            ("Length", "L"),
            ("Undo", "U"),
            ("Width", "W"),
        ]
    };
    names.iter().fold(options, |options, &(label, key)| {
        let allowed = match label {
            "Undo" => points > 1,
            "Close" | "CLose" => points > 2,
            _ => true,
        };
        if allowed {
            options.keyword(label, key)
        } else {
            options
        }
    })
}

/// Ask once and apply the answer.
fn step(ed: &Editor, chain: &mut Chain, mode: &mut Mode) -> Result<Flow, Cancelled> {
    let from = chain.last();
    let heading = chain.heading();
    let (shown, arcs, widths) = (chain.clone(), mode.arcs, mode.widths);
    let options = PointOptions::new(if arcs {
        "Specify endpoint of arc"
    } else {
        "Specify next point"
    })
    .base(from)
    .snap_to(chain.drawn())
    .ghost(move |at| {
        let curve = if arcs {
            bulge::tangent(from, heading, at).unwrap_or(0.0)
        } else {
            0.0
        };
        vec![shown.with_segment(at, curve, widths)]
    });
    let options = keywords(options, arcs, chain.points.len());
    match ed.ask_point(options)? {
        Answer::Enter => Ok(Flow::Finish),
        Answer::Value(to) => {
            if distance(from, to) > MIN_SIZE {
                let curve = if arcs {
                    bulge::tangent(from, heading, to).unwrap_or(0.0)
                } else {
                    0.0
                };
                chain.push(to, curve, mode.widths);
            }
            Ok(Flow::Next)
        }
        Answer::Keyword(k) => keyword(ed, &k, chain, mode, (from, heading)),
    }
}

fn keyword(
    ed: &Editor,
    keyword: &str,
    chain: &mut Chain,
    mode: &mut Mode,
    (from, heading): (Point, Point),
) -> Result<Flow, Cancelled> {
    match keyword {
        "Arc" => mode.arcs = true,
        "Line" => mode.arcs = false,
        "Undo" => chain.undo(),
        "Close" | "CLose" => {
            // The closing segment is an arc too when arcs are on.
            let first = chain.points[0];
            let curve = if mode.arcs {
                bulge::tangent(from, heading, first).unwrap_or(0.0)
            } else {
                0.0
            };
            chain.finish(curve, mode.widths);
            return Ok(Flow::Close);
        }
        "Width" => mode.widths = ask_widths(ed, from, mode.widths, false)?,
        "Halfwidth" => mode.widths = ask_widths(ed, from, mode.widths, true)?,
        "Length" => {
            let options = NumberOptions::new("Specify length of line").base(from);
            let length = ed.get_distance(options)?;
            if length > MIN_SIZE {
                let to = Point::new(from.x + heading.x * length, from.y + heading.y * length);
                chain.push(to, 0.0, mode.widths);
            } else {
                ed.say("Requires a non-zero value.");
            }
        }
        arc_keyword => match arc_option(ed, arc_keyword, from, heading)? {
            Some((to, curve)) if distance(from, to) > MIN_SIZE => {
                chain.push(to, curve, mode.widths);
            }
            _ => ed.say("Those values make no arc."),
        },
    }
    Ok(Flow::Next)
}

/// Ask for the widths at the start and end of the next segments, or half of
/// them. The start shows the current width; the end defaults to the start.
fn ask_widths(
    ed: &Editor,
    from: Point,
    current: (f64, f64),
    half: bool,
) -> Result<(f64, f64), Cancelled> {
    let scale = if half { 2.0 } else { 1.0 };
    let name = if half { "half-width" } else { "width" };
    let start = ed.get_distance(
        NumberOptions::new(format!("Specify starting {name}"))
            .default(current.0 / scale)
            .base(from),
    )?;
    let end = ed.get_distance(
        NumberOptions::new(format!("Specify ending {name}"))
            .default(start)
            .base(from),
    )?;
    Ok((start.max(0.0) * scale, end.max(0.0) * scale))
}

#[cfg(test)]
mod tests {
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
        state.start_command("Test", draw_polyline);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    type Parts = (Vec<Point>, Vec<f64>, Vec<(f64, f64)>, bool);

    fn polyline(e: &Entity) -> Parts {
        let Entity::Polyline {
            points,
            bulges,
            widths,
            closed,
        } = e
        else {
            panic!("a polyline, got {e:?}")
        };
        (points.clone(), bulges.clone(), widths.clone(), *closed)
    }

    fn labels(state: &mut AppState) -> Vec<String> {
        let options = state.cmd_prompt().options;
        options.iter().map(|o| o.label.to_string()).collect()
    }

    #[test]
    fn the_segments_drawn_so_far_are_snap_geometry() {
        let mut state = AppState::new();
        run(&mut state, &["0,0"]);
        assert!(state.waiting().unwrap().snap_geometry.is_empty());
        run_more(&mut state, &["10,0", "10,5"]);
        let drawn = state.waiting().unwrap().snap_geometry.clone();
        assert_eq!(drawn.len(), 1);
        assert_eq!(
            polyline(&drawn[0]).0,
            [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)]
        );
        run_more(&mut state, &["u"]);
        let drawn = state.waiting().unwrap().snap_geometry.clone();
        assert_eq!(polyline(&drawn[0]).0, [p(0.0, 0.0), p(10.0, 0.0)]);
    }

    #[test]
    fn polyline_is_one_object_ended_by_enter() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "10,0", "10,5", "5,5", "u", "10,10"]);
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(
            entities(&state),
            [Entity::Polyline {
                points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0), p(10.0, 10.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            }]
        );
        state.undo();
        assert!(entities(&state).is_empty(), "one undo step");
    }

    #[test]
    fn the_prompts_list_the_options_of_each_mode() {
        let mut state = AppState::new();
        run(&mut state, &["0,0"]);
        assert_eq!(labels(&mut state), ["Arc", "Halfwidth", "Length", "Width"]);
        run_more(&mut state, &["5,0", "5,5"]);
        assert_eq!(
            labels(&mut state),
            ["Arc", "Close", "Halfwidth", "Length", "Undo", "Width"]
        );
        run_more(&mut state, &["a"]);
        assert_eq!(
            labels(&mut state),
            [
                "Angle",
                "CEnter",
                "CLose",
                "Direction",
                "Halfwidth",
                "Line",
                "Radius",
                "Second pt",
                "Undo",
                "Width"
            ]
        );
    }

    fn run_more(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn polyline_close_joins_the_end_to_the_start() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "4,0", "4,3", "c"]);
        assert!(!state.command_running());
        assert!(polyline(&entities(&state)[0]).3);
    }

    #[test]
    fn polyline_with_a_single_point_draws_nothing() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", ""]);
        assert!(!state.command_running());
        assert!(entities(&state).is_empty());
    }

    #[test]
    fn polyline_arc_segments_run_on_smoothly_from_the_segment_before() {
        let mut state = AppState::new();
        // East to (10, 0), then an arc: it leaves heading east and ends at
        // (20, 10), a quarter circle about (10, 10). Then back to lines.
        run(&mut state, &["0,0", "10,0", "a", "20,10", "l", "20,30", ""]);
        assert!(!state.command_running());
        let all = entities(&state);
        assert_eq!(all.len(), 1);
        let (points, bulges, _, closed) = polyline(&all[0]);
        assert!(!closed);
        assert_eq!(
            points,
            vec![p(0.0, 0.0), p(10.0, 0.0), p(20.0, 10.0), p(20.0, 30.0)]
        );
        assert!(
            bulges[0].abs() < 1e-12 && bulges[2].abs() < 1e-12,
            "{bulges:?}"
        );
        assert!((bulges[1] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
    }

    #[test]
    fn polyline_arcs_can_be_undone_and_the_close_can_be_an_arc() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "a", "4,4", "u", "10,0", ""]);
        // The undone arc left no trace. Arc mode stayed on, but a point straight
        // ahead of the start is no arc, so the segment is straight.
        let (points, bulges, ..) = polyline(&entities(&state)[0]);
        assert_eq!(points, vec![p(0.0, 0.0), p(10.0, 0.0)]);
        assert!(bulges.is_empty());
        // A closed polyline with arcs on closes with an arc.
        let mut state = AppState::new();
        run(&mut state, &["0,0", "10,0", "a", "10,10", "cl"]);
        let (_, bulges, _, closed) = polyline(&entities(&state)[0]);
        assert!(closed);
        assert!(
            bulges[2].abs() > 1e-9,
            "the closing segment is an arc: {bulges:?}"
        );
    }

    #[test]
    fn width_sets_the_start_and_end_width_of_the_segments_that_follow() {
        let mut state = AppState::new();
        run(
            &mut state,
            &["0,0", "5,0", "w", "1", "3", "10,0", "w", "", "", "10,5", ""],
        );
        let (_, _, widths, _) = polyline(&entities(&state)[0]);
        // The first segment was drawn before any width; the second tapers 1 to
        // 3; the third keeps the start width for both ends (Enter, Enter).
        assert_eq!(widths, vec![(0.0, 0.0), (1.0, 3.0), (1.0, 1.0), (0.0, 0.0)]);
    }

    #[test]
    fn halfwidth_is_half_the_width() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "h", "0.5", "1", "5,0", ""]);
        let (_, _, widths, _) = polyline(&entities(&state)[0]);
        assert_eq!(widths, vec![(1.0, 2.0), (0.0, 0.0)]);
    }

    #[test]
    fn width_prompts_show_the_current_value_as_the_default() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "w"]);
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify starting width"));
        run_more(&mut state, &["2"]);
        assert!(state.cmd_prompt().text.starts_with("Specify ending width"));
        assert!(
            state.cmd_prompt().text.contains("<2"),
            "{}",
            state.cmd_prompt().text
        );
    }

    #[test]
    fn length_continues_in_the_direction_of_the_last_segment() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "0,5", "l", "3", ""]);
        let (points, ..) = polyline(&entities(&state)[0]);
        assert_eq!(points, vec![p(0.0, 0.0), p(0.0, 5.0), p(0.0, 8.0)]);
    }

    #[test]
    fn arc_second_point_gives_a_three_point_segment() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "a", "s", "2,2", "4,0", ""]);
        let (points, bulges, ..) = polyline(&entities(&state)[0]);
        assert_eq!(points, vec![p(0.0, 0.0), p(4.0, 0.0)]);
        // A half circle that bulges to the right of the way it travels: -1.
        assert!((bulges[0] + 1.0).abs() < 1e-9, "{bulges:?}");
    }

    #[test]
    fn arc_centre_angle_direction_and_radius_all_make_segments() {
        let mut state = AppState::new();
        // Centre (0,0), the arc from (5,0) counter-clockwise to the direction of (0,9).
        run(&mut state, &["5,0", "a", "ce", "0,0", "0,9", ""]);
        let (points, bulges, ..) = polyline(&entities(&state)[0]);
        assert!((points[1].x).abs() < 1e-9 && (points[1].y - 5.0).abs() < 1e-9);
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        // Included angle 90, then the end point.
        let mut state = AppState::new();
        run(&mut state, &["5,0", "a", "a", "90", "0,5", ""]);
        let (_, bulges, ..) = polyline(&entities(&state)[0]);
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        // Start direction north, end (0,5): a quarter circle about the origin.
        let mut state = AppState::new();
        run(&mut state, &["5,0", "a", "d", "90", "0,5", ""]);
        let (_, bulges, ..) = polyline(&entities(&state)[0]);
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        // Radius 5 to (0,5): the short arc, bending towards the end point's side.
        let mut state = AppState::new();
        run(&mut state, &["5,0", "a", "r", "5", "0,5", ""]);
        let (_, bulges, ..) = polyline(&entities(&state)[0]);
        assert!((bulges[0].abs() - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
    }

    #[test]
    fn an_arc_option_that_makes_no_arc_says_so_and_asks_again() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "a", "r", "1", "10,0"]);
        assert!(state.cmdline.log.iter().any(|l| l.contains("no arc")));
        assert!(state.command_running());
    }
}
