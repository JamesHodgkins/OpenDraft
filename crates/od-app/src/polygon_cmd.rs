//! POLYGON: a regular polygon about a centre, inscribed in or circumscribed
//! about a circle, or on an edge, as a closed polyline. The geometry is
//! `od_core::polygon`.

use std::sync::Mutex;

use od_commands::{Answer, Cancelled, Editor, NumberOptions, NumberOrPoint, Options, PointOptions};
use od_core::polygon::{about_point, about_radius, on_edge, Fit, MAX_SIDES, MIN_SIDES};
use od_core::{Entity, Point};

use crate::draw::add;

/// What POLYGON remembers between uses this session: the number of sides
/// (`POLYSIDES`) and whether the last one was inscribed.
static MEMORY: Mutex<(usize, Fit)> = Mutex::new((4, Fit::Inscribed));

/// `POLYSIDES`: the number of sides POLYGON offers first.
pub fn sides() -> usize {
    MEMORY.lock().unwrap_or_else(|e| e.into_inner()).0
}

pub fn set_sides(sides: usize) {
    MEMORY.lock().unwrap_or_else(|e| e.into_inner()).0 = sides;
}

fn last_fit() -> Fit {
    MEMORY.lock().unwrap_or_else(|e| e.into_inner()).1
}

fn set_fit(fit: Fit) {
    MEMORY.lock().unwrap_or_else(|e| e.into_inner()).1 = fit;
}

const INSCRIBED: &str = "Inscribed in circle";
const CIRCUMSCRIBED: &str = "Circumscribed about circle";

fn polygon(points: Vec<Point>) -> Entity {
    Entity::Polyline {
        points,
        bulges: Vec::new(),
        widths: Vec::new(),
        closed: true,
    }
}

/// The preview of `points`, or nothing while there is no polygon yet.
fn ghost(points: Option<Vec<Point>>) -> Vec<Entity> {
    points.map(polygon).into_iter().collect()
}

/// POLYGON: the number of sides, then a centre, Inscribed or Circumscribed
/// and a radius (typed: the bottom edge is level; picked: a vertex, or the
/// middle of an edge), or by the Edge option the two ends of one edge. One
/// undo step.
pub fn draw_polygon(ed: &mut Editor) -> Result<(), Cancelled> {
    let sides = ask_sides(ed)?;
    let options = PointOptions::new("Specify center of polygon").keyword("Edge", "E");
    let points = match ed.ask_point(options)? {
        Answer::Value(center) => about_center(ed, center, sides)?,
        Answer::Keyword(_) => by_edge(ed, sides)?,
        Answer::Enter => return Err(Cancelled),
    };
    add(ed, polygon(points), "Polygon")
}

fn ask_sides(ed: &Editor) -> Result<usize, Cancelled> {
    loop {
        let options = NumberOptions::new("Enter number of sides").default(sides() as f64);
        let typed = ed.get_number(options)?;
        let whole = typed.round();
        if (typed - whole).abs() < 1e-9 && (MIN_SIDES as f64..=MAX_SIDES as f64).contains(&whole) {
            let sides = whole as usize;
            set_sides(sides);
            return Ok(sides);
        }
        ed.say(format!(
            "Requires an integer between {MIN_SIDES} and {MAX_SIDES}."
        ));
    }
}

fn about_center(ed: &Editor, center: Point, sides: usize) -> Result<Vec<Point>, Cancelled> {
    let default = match last_fit() {
        Fit::Inscribed => INSCRIBED,
        Fit::Circumscribed => CIRCUMSCRIBED,
    };
    let chosen = ed.get_keyword(
        Options::new("Enter an option")
            .keyword(INSCRIBED, "I")
            .keyword(CIRCUMSCRIBED, "C")
            .default(default.to_owned()),
    )?;
    let fit = if chosen == CIRCUMSCRIBED {
        Fit::Circumscribed
    } else {
        Fit::Inscribed
    };
    set_fit(fit);
    loop {
        let options = NumberOptions::new("Specify radius of circle")
            .base(center)
            .ghost(move |at| ghost(about_point(center, at, sides, fit)));
        let made = match ed.ask_distance_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(radius)) => {
                about_radius(center, radius, sides, fit)
            }
            Answer::Value(NumberOrPoint::Point(at)) => about_point(center, at, sides, fit),
            _ => return Err(Cancelled),
        };
        match made {
            Some(points) => return Ok(points),
            None => ed.say("Requires a non-zero value."),
        }
    }
}

fn by_edge(ed: &Editor, sides: usize) -> Result<Vec<Point>, Cancelled> {
    let first = ed.get_point("Specify first endpoint of edge")?;
    loop {
        let options = PointOptions::new("Specify second endpoint of edge")
            .base(first)
            .ghost(move |at| ghost(on_edge(first, at, sides)));
        let second = ed.get_point(options)?;
        match on_edge(first, second, sides) {
            Some(points) => return Ok(points),
            None => ed.say("The two endpoints must differ."),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_commands::Reply;

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn drawing(answers: &[Reply]) -> AppState {
        let mut state = AppState::new();
        state.start_command("POLYGON", draw_polygon);
        state.settle_command();
        for answer in answers {
            state.reply(answer.clone());
            state.settle_command();
        }
        state
    }

    fn only_polygon(state: &AppState) -> Vec<Point> {
        let objects = state.session().document().objects();
        assert_eq!(objects.len(), 1);
        match &objects[0].entity {
            Entity::Polyline {
                points,
                closed: true,
                ..
            } => points.clone(),
            other => panic!("not a closed polyline: {other:?}"),
        }
    }

    #[test]
    fn a_centre_and_a_picked_vertex_make_an_inscribed_polygon() {
        let state = drawing(&[
            Reply::Number(6.0),
            Reply::Point(p(10.0, 10.0)),
            Reply::Keyword(INSCRIBED.to_owned()),
            Reply::Point(p(15.0, 10.0)),
        ]);
        let points = only_polygon(&state);
        assert_eq!(points.len(), 6);
        assert!((points[0].x - 15.0).abs() < 1e-9 && (points[0].y - 10.0).abs() < 1e-9);
        assert!(!state.command_running());
    }

    #[test]
    fn a_typed_radius_and_circumscribed_leave_the_bottom_edge_level() {
        let state = drawing(&[
            Reply::Number(4.0),
            Reply::Point(p(0.0, 0.0)),
            Reply::Keyword(CIRCUMSCRIBED.to_owned()),
            Reply::Number(5.0),
        ]);
        let points = only_polygon(&state);
        assert!((points[0].x + 5.0).abs() < 1e-9 && (points[0].y + 5.0).abs() < 1e-9);
        assert!((points[1].x - 5.0).abs() < 1e-9 && (points[1].y + 5.0).abs() < 1e-9);
    }

    #[test]
    fn the_edge_option_builds_on_two_endpoints_and_bad_sides_are_asked_again() {
        let mut state = drawing(&[Reply::Number(2.0)]);
        let said = state.cmdline.log.last().cloned().unwrap_or_default();
        assert_eq!(said, "Requires an integer between 3 and 1024.");
        for answer in [
            Reply::Number(3.0),
            Reply::Keyword("Edge".to_owned()),
            Reply::Point(p(0.0, 0.0)),
            Reply::Point(p(4.0, 0.0)),
        ] {
            state.reply(answer);
            state.settle_command();
        }
        let points = only_polygon(&state);
        assert_eq!(points.len(), 3);
        assert!(points[2].y > 3.4, "above the edge");
        state.undo();
        assert!(state.session().document().is_empty(), "one undo step");
    }
}
