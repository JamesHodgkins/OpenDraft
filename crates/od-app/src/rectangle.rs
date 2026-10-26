//! RECTANGLE: two opposite corners, or a size and then which way it lies, at
//! any angle, with square, rounded or bevelled corners and a line width.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::rect::{outline, Corners};
use od_core::{Entity, Point};

use crate::draw::{add, MIN_SIZE};
use crate::rect_memory::{memory, remember};

/// The corner style the remembered settings call for.
fn corners() -> Corners {
    let m = memory();
    if m.radius > 0.0 {
        Corners::Fillet(m.radius)
    } else {
        Corners::Chamfer(m.first, m.second)
    }
}

/// A size given by Area or Dimensions: along the rectangle's own x and y.
type Size = (f64, f64);

/// Where the opposite corner is, in the rectangle's own axes, from the first
/// corner: the picked point's offset, or the given size on the side of the
/// first corner the pick lies on.
fn extent(first: Point, angle: f64, size: Option<Size>, at: Point) -> (f64, f64) {
    let (rx, ry) = (at.x - first.x, at.y - first.y);
    let (x, y) = (
        rx * angle.cos() + ry * angle.sin(),
        -rx * angle.sin() + ry * angle.cos(),
    );
    match size {
        None => (x, y),
        Some((length, width)) => (
            if x < 0.0 { -length } else { length },
            if y < 0.0 { -width } else { width },
        ),
    }
}

/// The remembered value changed by a first-corner option.
fn set_option(ed: &Editor, option: &str) -> Result<(), Cancelled> {
    let m = memory();
    let size = |message: &str, default: f64| {
        ed.get_distance(NumberOptions::new(message).default(default))
            .map(|v| v.max(0.0))
    };
    match option {
        "Chamfer" => {
            let first = size("Specify first chamfer distance for rectangles", m.first)?;
            let second = size("Specify second chamfer distance for rectangles", first)?;
            // Bevels and rounding exclude each other.
            remember(|m| (m.first, m.second, m.radius) = (first, second, 0.0));
        }
        "Fillet" => {
            let radius = size("Specify fillet radius for rectangles", m.radius)?;
            remember(|m| (m.radius, m.first, m.second) = (radius, 0.0, 0.0));
        }
        "Width" => {
            let width = size("Specify line width for rectangles", m.width)?;
            remember(|m| m.width = width);
        }
        "Elevation" => {
            let value = ed.get_number(
                NumberOptions::new("Specify the elevation for rectangles").default(m.elevation),
            )?;
            remember(|m| m.elevation = value);
        }
        _ => {
            let value = ed.get_number(
                NumberOptions::new("Specify the thickness for rectangles").default(m.thickness),
            )?;
            remember(|m| m.thickness = value);
        }
    }
    Ok(())
}

fn positive(ed: &Editor, message: &str) -> Result<f64, Cancelled> {
    loop {
        let value = ed.get_distance(message)?;
        if value > MIN_SIZE {
            return Ok(value);
        }
        ed.say("Requires a non-zero value.");
    }
}

/// Area: the area, then the length or the width, give the other side.
fn size_from_area(ed: &Editor) -> Result<Size, Cancelled> {
    let area = loop {
        let area = ed.get_number("Enter area of rectangle in current units")?;
        if area > MIN_SIZE {
            break area;
        }
        ed.say("Requires a positive value.");
    };
    let by = ed.get_keyword(
        Options::new("Calculate rectangle dimensions based on")
            .keyword("Length", "L")
            .keyword("Width", "W")
            .default("Length".to_owned()),
    )?;
    Ok(if by == "Length" {
        let length = positive(ed, "Enter rectangle length")?;
        (length, area / length)
    } else {
        let width = positive(ed, "Enter rectangle width")?;
        (area / width, width)
    })
}

/// Rotation: an angle, typed or picked from the first corner, or two points.
fn rotation(ed: &Editor, first: Point, current: f64) -> Result<f64, Cancelled> {
    let options = NumberOptions::new("Specify rotation angle")
        .default(current)
        .base(first)
        .keyword("Pick points", "P");
    Ok(match ed.ask_angle(options)? {
        Answer::Value(angle) => angle,
        Answer::Keyword(_) => {
            let a = ed.get_point("Specify first point")?;
            let b = ed.get_point(PointOptions::new("Specify second point").base(a))?;
            (b.y - a.y).atan2(b.x - a.x)
        }
        Answer::Enter => return Err(Cancelled),
    })
}

/// RECTANGLE. The first corner prompt also sets what the next rectangles look
/// like (Chamfer, Fillet, Width, and Elevation and Thickness, which are kept
/// but not drawn in 2D). After it, Area and Dimensions give a size and the
/// pointer chooses which of the four ways it lies from the first corner, and
/// Rotation turns the whole rectangle about the first corner.
pub fn draw_rectangle(ed: &mut Editor) -> Result<(), Cancelled> {
    let first = loop {
        let options = PointOptions::new("Specify first corner point")
            .keyword("Chamfer", "C")
            .keyword("Elevation", "E")
            .keyword("Fillet", "F")
            .keyword("Thickness", "T")
            .keyword("Width", "W");
        match ed.ask_point(options)? {
            Answer::Value(first) => break first,
            Answer::Keyword(option) => set_option(ed, &option)?,
            Answer::Enter => return Err(Cancelled),
        }
    };
    let (style, width) = (corners(), memory().width);
    let (mut angle, mut size): (f64, Option<Size>) = (0.0, None);
    let (dx, dy) = loop {
        let options = PointOptions::new("Specify other corner point")
            .keyword("Area", "A")
            .keyword("Dimensions", "D")
            .keyword("Rotation", "R")
            .ghost(move |at| {
                let (dx, dy) = extent(first, angle, size, at);
                vec![outline(first, angle, dx, dy, style, width)]
            });
        match ed.ask_corner(first, options)? {
            Answer::Value(at) => {
                let (dx, dy) = extent(first, angle, size, at);
                if dx.abs() > MIN_SIZE && dy.abs() > MIN_SIZE {
                    break (dx, dy);
                }
                ed.say("Requires a non-zero value.");
            }
            Answer::Keyword(k) if k == "Area" => size = Some(size_from_area(ed)?),
            Answer::Keyword(k) if k == "Dimensions" => {
                let length = positive(ed, "Specify length for rectangles")?;
                let width = positive(ed, "Specify width for rectangles")?;
                size = Some((length, width));
            }
            Answer::Keyword(_) => angle = rotation(ed, first, angle)?,
            Answer::Enter => return Err(Cancelled),
        }
    };
    let entity: Entity = outline(first, angle, dx, dy, style, width);
    add(ed, entity, "Rectangle")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect_memory::take_turn_for_tests as take_turn;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_command("Test", draw_rectangle);
        state.settle_command();
        run_more(state, inputs);
    }

    fn run_more(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn labels(state: &mut AppState) -> Vec<String> {
        let options = state.cmd_prompt().options;
        options.iter().map(|o| o.label.to_string()).collect()
    }

    fn rect(min: Point, max: Point) -> Entity {
        Entity::Rectangle { min, max }
    }

    fn points_of(e: &Entity) -> Vec<Point> {
        let Entity::Polyline { points, .. } = e else {
            panic!("a polyline, got {e:?}")
        };
        points.clone()
    }

    #[test]
    fn the_prompts_offer_the_options_of_each_stage() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &[]);
        assert_eq!(
            labels(&mut state),
            ["Chamfer", "Elevation", "Fillet", "Thickness", "Width"]
        );
        run_more(&mut state, &["0,0"]);
        assert_eq!(labels(&mut state), ["Area", "Dimensions", "Rotation"]);
    }

    #[test]
    fn rectangle_takes_corners_in_any_order() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["10,10", "2,20"]);
        assert_eq!(entities(&state), [rect(p(2.0, 10.0), p(10.0, 20.0))]);
    }

    #[test]
    fn dimensions_ask_for_length_and_width_then_the_pointer_picks_the_quadrant() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["2,3", "d"]);
        assert_eq!(state.cmd_prompt().text, "Specify length for rectangles");
        run_more(&mut state, &["10", "4"]);
        assert!(
            state.command_running(),
            "waiting to be told which way it lies"
        );
        assert_eq!(labels(&mut state), ["Area", "Dimensions", "Rotation"]);
        run_more(&mut state, &["-1,-1"]);
        assert_eq!(entities(&state), [rect(p(-8.0, -1.0), p(2.0, 3.0))]);
    }

    #[test]
    fn area_and_a_length_give_the_width() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "a", "40"]);
        assert!(state.cmd_prompt().text.contains("<Length>"));
        run_more(&mut state, &["", "10", "50,50"]);
        assert_eq!(entities(&state), [rect(p(0.0, 0.0), p(10.0, 4.0))]);
        run(&mut state, &["0,0", "a", "40", "w", "5", "50,50"]);
        assert_eq!(entities(&state)[1], rect(p(0.0, 0.0), p(8.0, 5.0)));
    }

    #[test]
    fn rotation_turns_the_rectangle_about_the_first_corner() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "r", "90", "-2,4"]);
        let points = points_of(&entities(&state)[0]);
        // In its own axes the far corner is (4, 2): up the y axis, then left.
        assert!((points[2].x + 2.0).abs() < 1e-9 && (points[2].y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rotation_by_picking_two_points() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "r", "p", "0,0", "0,5", "-2,4"]);
        let points = points_of(&entities(&state)[0]);
        assert!((points[2].x + 2.0).abs() < 1e-9 && (points[2].y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn fillet_and_chamfer_are_remembered_and_exclude_each_other() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["f", "1", "0,0", "10,6"]);
        let Entity::Polyline { points, bulges, .. } = &entities(&state)[0] else {
            panic!("rounded")
        };
        assert_eq!(points.len(), 8);
        assert!(!bulges.is_empty());
        // Next time the radius is the default; a chamfer replaces it.
        run(&mut state, &["f"]);
        assert!(
            state.cmd_prompt().text.contains("<1"),
            "{}",
            state.cmd_prompt().text
        );
        state.cancel();
        run(&mut state, &["c", "2", "1", "0,0", "10,6"]);
        let Entity::Polyline { points, bulges, .. } = &entities(&state)[1] else {
            panic!("bevelled")
        };
        assert_eq!(points.len(), 8);
        assert!(bulges.is_empty(), "straight bevels, no arcs");
        assert_eq!(memory().radius, 0.0);
    }

    #[test]
    fn width_makes_a_wide_polyline_and_is_remembered() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["w", "0.5", "0,0", "4,2"]);
        let Entity::Polyline { widths, closed, .. } = &entities(&state)[0] else {
            panic!("wide")
        };
        assert!(closed);
        assert_eq!(widths, &vec![(0.5, 0.5); 4]);
        run(&mut state, &["0,0", "4,2"]);
        assert!(
            matches!(entities(&state)[1], Entity::Polyline { .. }),
            "still wide"
        );
    }

    #[test]
    fn elevation_and_thickness_are_kept_for_the_variables_but_draw_nothing() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["e", "-2", "t", "3", "0,0", "4,2"]);
        assert_eq!(entities(&state), [rect(p(0.0, 0.0), p(4.0, 2.0))]);
        assert_eq!((memory().elevation, memory().thickness), (-2.0, 3.0));
    }

    #[test]
    fn a_flat_rectangle_is_refused() {
        let _turn = take_turn();
        let mut state = AppState::new();
        run(&mut state, &["0,0", "5,0"]);
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
    }
}
