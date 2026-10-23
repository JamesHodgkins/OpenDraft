//! DIMANGULAR: the angle of an arc, between two points of a circle, between
//! two lines, or at a vertex between two points. The geometry (and which of
//! the angles the rays make is measured) is `od_core::dimension`; this asks.

use std::f64::consts::TAU;
use std::sync::Arc;

use od_commands::{Cancelled, Editor, PointOptions};
use od_core::dimension::{lines_angle_points, object_ends};
use od_core::{Dimension, Entity, Point};

use crate::dimension_cmd::{pick, place, style, Make};

/// The three points an angular dimension is built on, and whether the rays
/// are whole lines.
struct Rays {
    vertex: Point,
    p1: Point,
    p2: Point,
    lines: bool,
    /// For an arc: the angles it spans, so the dimension arc stays within.
    span: Option<(f64, f64)>,
}

fn angle(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

/// Pick what to dimension; `None` when the user chose to give three points.
fn rays(ed: &mut Editor) -> Result<Rays, Cancelled> {
    loop {
        let Some((picked, entity)) = pick(ed, "Select arc, circle, line, or <specify vertex>")?
        else {
            return three_points(ed);
        };
        match entity {
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let at = |a: f64| od_core::arc::point_at(center, radius, a);
                return Ok(Rays {
                    vertex: center,
                    p1: at(start),
                    p2: at(end),
                    lines: false,
                    span: Some((start, (end - start).rem_euclid(TAU))),
                });
            }
            Entity::Circle { center, .. } => {
                let second =
                    ed.get_point(PointOptions::new("Specify second angle endpoint").base(center))?;
                return Ok(Rays {
                    vertex: center,
                    p1: picked.at,
                    p2: second,
                    lines: false,
                    span: None,
                });
            }
            other => {
                let Some(first) = object_ends(&other, picked.at) else {
                    ed.say("Select an arc, a circle or a line.");
                    continue;
                };
                return second_line(ed, first);
            }
        }
    }
}

fn second_line(ed: &mut Editor, first: (Point, Point)) -> Result<Rays, Cancelled> {
    loop {
        let Some((picked, entity)) = pick(ed, "Select second line")? else {
            return Err(Cancelled);
        };
        let Some(second) = object_ends(&entity, picked.at) else {
            ed.say("Select a line.");
            continue;
        };
        match lines_angle_points(first, second) {
            Some((vertex, p1, p2)) => {
                return Ok(Rays {
                    vertex,
                    p1,
                    p2,
                    lines: true,
                    span: None,
                })
            }
            None => ed.say("The lines are parallel."),
        }
    }
}

fn three_points(ed: &mut Editor) -> Result<Rays, Cancelled> {
    let vertex = ed.get_point(PointOptions::new("Specify angle vertex"))?;
    let p1 = ed.get_point(PointOptions::new("Specify first angle endpoint").base(vertex))?;
    let p2 = ed.get_point(PointOptions::new("Specify second angle endpoint").base(vertex))?;
    Ok(Rays {
        vertex,
        p1,
        p2,
        lines: false,
        span: None,
    })
}

/// DIMANGULAR.
pub fn dim_angular(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let rays = rays(ed)?;
    let Rays {
        vertex,
        p1,
        p2,
        lines,
        span,
    } = rays;
    let make: Make = Arc::new(move |at, tweaks| {
        // Dimensioning an arc measures the arc, wherever the arc is put.
        let at = match span {
            Some((start, sweep)) if (angle(vertex, at) - start).rem_euclid(TAU) > sweep => {
                let r = ((at.x - vertex.x).hypot(at.y - vertex.y)).max(1e-9);
                let mid = start + sweep / 2.0;
                Point::new(vertex.x + r * mid.cos(), vertex.y + r * mid.sin())
            }
            _ => at,
        };
        let mut dimension = Dimension::angular(vertex, p1, p2, at, lines, &style);
        dimension.text = tweaks.text.clone();
        dimension.measurement().map(|_| dimension)
    });
    let sample = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    place(
        ed,
        "Specify dimension arc line location",
        &[("Text", "T")],
        sample,
        make,
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn started(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("DIMANGULAR", dim_angular);
        state.settle_command();
        state
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), false);
        state.settle_command();
    }

    fn the_dimension(state: &AppState) -> Dimension {
        state
            .session()
            .document()
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn three_points_make_an_angle() {
        let mut state = started(&[]);
        submit(&mut state, "");
        submit(&mut state, "0,0");
        submit(&mut state, "10,0");
        submit(&mut state, "0,10");
        submit(&mut state, "7,7");
        let d = the_dimension(&state);
        assert!((d.measurement().unwrap() - FRAC_PI_2).abs() < 1e-9);
        assert_eq!(d.label(), "90");
    }

    #[test]
    fn two_lines_give_the_angle_chosen_by_the_pick() {
        let a = Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        };
        let b = Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 10.0),
        };
        let mut state = started(&[a, b]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(5.0, 5.0));
        submit(&mut state, "-5,5");
        let d = the_dimension(&state);
        assert_eq!(d.label(), "135");
    }

    #[test]
    fn an_arc_is_measured_whatever_side_the_arc_line_is_put() {
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        let mut state = started(&[arc]);
        click(&mut state, 0, p(7.0, 7.0));
        submit(&mut state, "-20,-20");
        assert_eq!(the_dimension(&state).label(), "90");
    }

    #[test]
    fn a_circle_takes_a_second_point() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 10.0,
        };
        let mut state = started(&[circle]);
        click(&mut state, 0, p(10.0, 0.0));
        submit(&mut state, "0,10");
        submit(&mut state, "14,14");
        assert_eq!(the_dimension(&state).label(), "90");
    }

    #[test]
    fn parallel_lines_are_refused_and_asked_again() {
        let a = Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        };
        let b = Entity::Line {
            start: p(0.0, 5.0),
            end: p(10.0, 5.0),
        };
        let mut state = started(&[a, b]);
        click(&mut state, 0, p(5.0, 0.0));
        click(&mut state, 1, p(5.0, 5.0));
        assert!(state.cmdline.log.last().unwrap().contains("parallel"));
        assert!(state.command_running());
    }
}
