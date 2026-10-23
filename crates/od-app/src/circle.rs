//! CIRCLE: centre and radius or diameter, three points, two diameter ends, or
//! tangent to two objects with a radius.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::{arc, tangent_circle, Entity, Point};

use crate::draw::{add, distance, line, MIN_SIZE};

const TTR: &str = "Ttr (tan tan radius)";

fn circle(center: Point, radius: f64) -> Entity {
    Entity::Circle { center, radius }
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// The circle through three points, or a line to show while they are in a row.
fn circle_through(a: Point, b: Point, c: Point) -> Entity {
    match arc::through_points(a, b, c) {
        Some((center, radius, ..)) => circle(center, radius),
        None => line(a, c),
    }
}

/// CIRCLE: a center, then a radius (or a diameter, by option); or by option
/// three points on the circle, the two ends of a diameter, or two tangent
/// objects and a radius.
pub fn draw_circle(ed: &mut Editor) -> Result<(), Cancelled> {
    let options = PointOptions::new("Specify center point for circle")
        .keyword("3P", "3P")
        .keyword("2P", "2P")
        .keyword(TTR, "T");
    let center = match ed.ask_point(options)? {
        Answer::Value(center) => center,
        Answer::Keyword(k) if k == "3P" => return three_points(ed),
        Answer::Keyword(k) if k == "2P" => return two_points(ed),
        Answer::Keyword(_) => return tangent_tangent_radius(ed),
        Answer::Enter => return Err(Cancelled),
    };
    let radius = loop {
        let options = NumberOptions::new("Specify radius of circle")
            .base(center)
            .keyword("Diameter", "D")
            .ghost(move |at| vec![circle(center, distance(center, at))]);
        let radius = match ed.ask_distance(options)? {
            Answer::Value(radius) => radius,
            Answer::Keyword(_) => {
                let options = NumberOptions::new("Specify diameter of circle")
                    .base(center)
                    .ghost(move |at| vec![circle(center, distance(center, at) / 2.0)]);
                ed.get_distance(options)? / 2.0
            }
            Answer::Enter => return Err(Cancelled),
        };
        if radius > MIN_SIZE {
            break radius;
        }
        ed.say("Requires a non-zero value.");
    };
    add(ed, circle(center, radius), "Circle")
}

/// CIRCLE, Center and Diameter: the centre, then the diameter.
pub fn draw_circle_diameter(ed: &mut Editor) -> Result<(), Cancelled> {
    let center = ed.get_point("Specify center point for circle")?;
    let radius = loop {
        let options = NumberOptions::new("Specify diameter of circle")
            .base(center)
            .ghost(move |at| vec![circle(center, distance(center, at) / 2.0)]);
        let radius = ed.get_distance(options)? / 2.0;
        if radius > MIN_SIZE {
            break radius;
        }
        ed.say("Requires a non-zero value.");
    };
    add(ed, circle(center, radius), "Circle")
}

/// CIRCLE, 2-Point: the two ends of a diameter.
pub fn draw_circle_2p(ed: &mut Editor) -> Result<(), Cancelled> {
    two_points(ed)
}

/// CIRCLE, 3-Point: three points on the circle.
pub fn draw_circle_3p(ed: &mut Editor) -> Result<(), Cancelled> {
    three_points(ed)
}

/// CIRCLE, Tan, Tan, Radius: two objects to touch, then a radius.
pub fn draw_circle_ttr(ed: &mut Editor) -> Result<(), Cancelled> {
    tangent_tangent_radius(ed)
}

fn three_points(ed: &Editor) -> Result<(), Cancelled> {
    let first = ed.get_point("Specify first point")?;
    let options = PointOptions::new("Specify second point")
        .base(first)
        .ghost(move |at| vec![line(first, at)]);
    let second = ed.get_point(options)?;
    let entity = loop {
        let options = PointOptions::new("Specify third point")
            .base(second)
            .ghost(move |at| vec![circle_through(first, second, at)]);
        let third = ed.get_point(options)?;
        let entity = circle_through(first, second, third);
        if matches!(entity, Entity::Circle { .. }) {
            break entity;
        }
        ed.say("The three points are in a line.");
    };
    add(ed, entity, "Circle")
}

fn two_points(ed: &Editor) -> Result<(), Cancelled> {
    let first = ed.get_point("Specify first end point of diameter")?;
    let entity = loop {
        let options = PointOptions::new("Specify second end point of diameter")
            .base(first)
            .ghost(move |at| vec![circle(midpoint(first, at), distance(first, at) / 2.0)]);
        let second = ed.get_point(options)?;
        if distance(first, second) > MIN_SIZE {
            break circle(midpoint(first, second), distance(first, second) / 2.0);
        }
        ed.say("Requires a non-zero value.");
    };
    add(ed, entity, "Circle")
}

/// Click an object; the object and where it was clicked.
fn pick_object(ed: &Editor, message: &str) -> Result<(Entity, Point), Cancelled> {
    loop {
        let Answer::Value(picked) = ed.ask_object(message)? else {
            return Err(Cancelled);
        };
        let id = picked.id;
        let entity = ed.read(move |doc| doc.object(id).map(|o| o.entity.clone()))?;
        if let Some(entity) = entity {
            return Ok((entity, picked.at));
        }
    }
}

fn tangent_tangent_radius(ed: &Editor) -> Result<(), Cancelled> {
    let (first, at_first) = pick_object(ed, "Specify point on object for first tangent")?;
    let (second, at_second) = pick_object(ed, "Specify point on object for second tangent")?;
    let radius = loop {
        let radius = ed.get_distance(NumberOptions::new("Specify radius of circle"))?;
        if radius > MIN_SIZE {
            break radius;
        }
        ed.say("Requires a non-zero value.");
    };
    match tangent_circle(&first, at_first, &second, at_second, radius) {
        Some(center) => add(ed, circle(center, radius), "Circle"),
        None => {
            ed.say("Circle does not exist.");
            Ok(())
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

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_command("Test", draw_circle);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn radius_and_centre(e: &Entity) -> (Point, f64) {
        let Entity::Circle { center, radius } = e else {
            panic!("expected a circle, got {e:?}")
        };
        (*center, *radius)
    }

    #[test]
    fn each_circle_type_starts_at_its_own_prompt() {
        let mut state = AppState::new();
        let mut start =
            |name: &str, run: fn(&mut Editor) -> Result<(), Cancelled>, inputs: &[&str]| {
                state.start_command(name, run);
                state.settle_command();
                for input in inputs {
                    state.cmd_submit(input);
                    state.settle_command();
                }
                assert!(!state.command_running(), "{name}");
            };
        start("Diameter", draw_circle_diameter, &["1,1", "8"]);
        start("2P", draw_circle_2p, &["0,0", "6,0"]);
        start("3P", draw_circle_3p, &["4,0", "0,4", "-4,0"]);
        let found = entities(&state);
        assert_eq!(found[0], circle(p(1.0, 1.0), 4.0));
        assert_eq!(found[1], circle(p(3.0, 0.0), 3.0));
        let (center, radius) = radius_and_centre(&found[2]);
        assert!(center.x.abs() < 1e-9 && center.y.abs() < 1e-9 && (radius - 4.0).abs() < 1e-9);
    }

    #[test]
    fn circle_takes_a_radius_or_a_diameter() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "5"]);
        assert!(!state.command_running(), "one circle per command");
        run(&mut state, &["0,0", "d", "8"]);
        assert_eq!(
            entities(&state),
            [circle(p(0.0, 0.0), 5.0), circle(p(0.0, 0.0), 4.0)]
        );
    }

    #[test]
    fn circle_by_clicking_uses_the_distance_from_the_center() {
        let mut state = AppState::new();
        state.start_command("Circle", draw_circle);
        state.settle_command();
        state.answer_point(p(1.0, 1.0));
        state.settle_command();
        state.answer_point(p(4.0, 5.0));
        state.settle_command();
        assert_eq!(entities(&state), [circle(p(1.0, 1.0), 5.0)]);
    }

    #[test]
    fn circle_refuses_a_zero_radius_and_asks_again() {
        let mut state = AppState::new();
        run(&mut state, &["0,0", "0"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
        let said = state.cmdline.log.iter().filter(|l| l.contains("non-zero"));
        assert_eq!(said.count(), 1);
    }

    #[test]
    fn the_first_prompt_offers_the_three_other_ways() {
        let mut state = AppState::new();
        run(&mut state, &[]);
        let labels: Vec<_> = state
            .cmd_prompt()
            .options
            .iter()
            .map(|o| o.label.to_string())
            .collect();
        assert_eq!(labels, ["3P", "2P", TTR]);
    }

    #[test]
    fn three_points_make_the_circle_through_them() {
        let mut state = AppState::new();
        run(&mut state, &["3p", "5,0", "0,5", "-5,0"]);
        let (centre, radius) = radius_and_centre(&entities(&state)[0]);
        assert!(centre.x.abs() < 1e-9 && centre.y.abs() < 1e-9);
        assert!((radius - 5.0).abs() < 1e-9);
        assert!(!state.command_running());
    }

    #[test]
    fn three_points_in_a_line_are_refused_and_asked_again() {
        let mut state = AppState::new();
        run(&mut state, &["3p", "0,0", "1,1", "2,2"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
        assert!(state.cmdline.log.iter().any(|l| l.contains("in a line")));
    }

    #[test]
    fn two_points_are_the_ends_of_a_diameter() {
        let mut state = AppState::new();
        run(&mut state, &["2p", "0,0", "10,0"]);
        assert_eq!(entities(&state), [circle(p(5.0, 0.0), 5.0)]);
    }

    #[test]
    fn two_points_on_the_same_spot_are_refused() {
        let mut state = AppState::new();
        run(&mut state, &["2p", "3,3", "3,3"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
    }

    #[test]
    fn ttr_fits_a_circle_between_two_lines() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        state.add_entity(line(p(0.0, 0.0), p(0.0, 10.0)));
        let ids: Vec<_> = state
            .session()
            .document
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        run(&mut state, &["t"]);
        state.answer_object(p(5.0, 0.0), Some(ids[0]), false);
        state.settle_command();
        state.answer_object(p(0.0, 5.0), Some(ids[1]), false);
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify radius of circle"));
        state.cmd_submit("2");
        state.settle_command();
        let all = entities(&state);
        let (centre, radius) = radius_and_centre(&all[2]);
        assert!((centre.x - 2.0).abs() < 1e-9 && (centre.y - 2.0).abs() < 1e-9);
        assert_eq!(radius, 2.0);
        assert!(!state.command_running());
    }

    #[test]
    fn ttr_says_when_no_circle_fits() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        state.add_entity(line(p(0.0, 10.0), p(10.0, 10.0)));
        let ids: Vec<_> = state
            .session()
            .document
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        run(&mut state, &["t"]);
        state.answer_object(p(5.0, 0.0), Some(ids[0]), false);
        state.settle_command();
        state.answer_object(p(5.0, 10.0), Some(ids[1]), false);
        state.settle_command();
        state.cmd_submit("1");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Circle does not exist")));
        assert!(!state.command_running());
    }
}
