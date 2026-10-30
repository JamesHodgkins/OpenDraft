//! The flange recipe, to the number. The sample is a made-up flange chosen
//! so the arithmetic is easy to check; it is not a standard's flange.

use super::{Shape, WeldNeckFlange};
use crate::Point;

fn sample() -> WeldNeckFlange {
    WeldNeckFlange {
        outside_diameter: 100.0,
        thickness: 14.0,
        raised_face_diameter: 60.0,
        raised_face_height: 2.0,
        hub_diameter_base: 50.0,
        hub_diameter_end: 38.0,
        overall_length: 60.0,
        bore: 30.0,
        bolt_circle_diameter: 75.0,
        bolt_count: 4,
        bolt_hole_diameter: 10.0,
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0e-9
}

fn outlines(shapes: &[Shape]) -> Vec<&Vec<Point>> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Outline(p) => Some(p),
            _ => None,
        })
        .collect()
}

fn circles(shapes: &[Shape]) -> Vec<(Point, f64)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Circle { center, radius } => Some((*center, *radius)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_pieces_are_the_two_section_halves_the_circles_and_two_centrelines() {
    let shapes = sample().shapes().unwrap();
    assert_eq!(outlines(&shapes).len(), 2);
    // Edge, raised face, bore, bolt circle, four holes.
    assert_eq!(circles(&shapes).len(), 8);
    assert_eq!(
        shapes
            .iter()
            .filter(|s| matches!(s, Shape::Line(..)))
            .count(),
        2
    );
}

#[test]
fn the_upper_half_of_the_section_follows_the_profile_exactly() {
    let shapes = sample().shapes().unwrap();
    let upper = outlines(&shapes)[0].clone();
    let expected = [
        (0.0, 15.0),
        (0.0, 30.0),
        (2.0, 30.0),
        (2.0, 50.0),
        (16.0, 50.0),
        (16.0, 25.0),
        (60.0, 19.0),
        (60.0, 15.0),
    ];
    assert_eq!(upper.len(), expected.len());
    for (got, want) in upper.iter().zip(expected) {
        assert!(
            near(got.x, want.0) && near(got.y, want.1),
            "{got:?} vs {want:?}"
        );
    }
}

#[test]
fn the_lower_half_is_the_mirror_of_the_upper() {
    let shapes = sample().shapes().unwrap();
    let both = outlines(&shapes);
    for (up, down) in both[0].iter().zip(both[1]) {
        assert!(near(up.x, down.x) && near(up.y, -down.y));
    }
}

#[test]
fn a_flat_face_has_no_raised_step_and_no_raised_circle() {
    let mut flange = sample();
    flange.raised_face_height = 0.0;
    let shapes = flange.shapes().unwrap();
    assert_eq!(outlines(&shapes)[0].len(), 6);
    assert_eq!(circles(&shapes).len(), 7);
}

#[test]
fn the_face_view_is_to_the_left_of_the_section_and_clear_of_it() {
    let flange = sample();
    let shapes = flange.shapes().unwrap();
    let face_centre = -(100.0 * 0.25 + 50.0);
    for (centre, radius) in circles(&shapes).iter().take(4) {
        assert!(near(centre.x, face_centre) && near(centre.y, 0.0));
        assert!(centre.x + radius <= face_centre + 50.0 + 1e-9);
    }
    let section_left = outlines(&shapes)
        .iter()
        .flat_map(|o| o.iter().map(|p| p.x))
        .fold(f64::MAX, f64::min);
    assert!(face_centre + 50.0 < section_left, "a gap between the views");
}

#[test]
fn the_holes_are_equally_spaced_on_the_bolt_circle() {
    let mut flange = sample();
    flange.bolt_count = 8;
    let shapes = flange.shapes().unwrap();
    let face_centre = -75.0;
    let holes: Vec<Point> = circles(&shapes)
        .into_iter()
        .filter(|(_, r)| near(*r, 5.0))
        .map(|(c, _)| c)
        .collect();
    assert_eq!(holes.len(), 8);
    for hole in &holes {
        let distance = ((hole.x - face_centre).powi(2) + hole.y.powi(2)).sqrt();
        assert!(near(distance, 37.5), "{distance}");
    }
    // Neighbouring holes are the same distance apart all the way round.
    let chord = |a: &Point, b: &Point| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
    let first = chord(&holes[0], &holes[1]);
    for pair in holes.windows(2) {
        assert!(near(chord(&pair[0], &pair[1]), first));
    }
}

#[test]
fn no_hole_is_on_a_centreline_and_the_pattern_is_symmetric_both_ways() {
    for count in [4, 8, 12, 16, 20] {
        let mut flange = sample();
        flange.bolt_count = count;
        // Room for twenty holes on this circle.
        flange.bolt_hole_diameter = 6.0;
        let shapes = flange.shapes().unwrap();
        let holes: Vec<Point> = circles(&shapes)
            .into_iter()
            .filter(|(_, r)| near(*r, 3.0))
            .map(|(c, _)| Point::new(c.x + 75.0, c.y))
            .collect();
        assert_eq!(holes.len(), count as usize);
        for h in &holes {
            assert!(
                h.x.abs() > 1e-6 && h.y.abs() > 1e-6,
                "{count}: {h:?} on a centreline"
            );
            let has = |x: f64, y: f64| holes.iter().any(|o| near(o.x, x) && near(o.y, y));
            assert!(has(-h.x, h.y) && has(h.x, -h.y), "{count}: not symmetric");
        }
    }
}

#[test]
fn an_odd_count_has_one_hole_at_the_top_and_is_symmetric_about_the_vertical() {
    let mut flange = sample();
    flange.bolt_count = 5;
    let shapes = flange.shapes().unwrap();
    let holes: Vec<Point> = circles(&shapes)
        .into_iter()
        .filter(|(_, r)| near(*r, 5.0))
        .map(|(c, _)| Point::new(c.x + 75.0, c.y))
        .collect();
    assert_eq!(holes.len(), 5);
    let on_axis: Vec<&Point> = holes.iter().filter(|h| h.x.abs() < 1e-6).collect();
    assert_eq!(on_axis.len(), 1);
    assert!(
        near(on_axis[0].y, 37.5),
        "the one on the axis is at the top"
    );
    for h in &holes {
        assert!(holes.iter().any(|o| near(o.x, -h.x) && near(o.y, h.y)));
    }
}

#[test]
fn the_centrelines_run_past_both_views() {
    let shapes = sample().shapes().unwrap();
    let lines: Vec<(Point, Point)> = shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Line(a, b) => Some((*a, *b)),
            _ => None,
        })
        .collect();
    let (axis, vertical) = (lines[0], lines[1]);
    assert!(near(axis.0.y, 0.0) && near(axis.1.y, 0.0));
    assert!(axis.0.x < -125.0 && axis.1.x > 60.0);
    assert!(near(vertical.0.x, -75.0) && near(vertical.1.x, -75.0));
    assert!(vertical.0.y < -50.0 && vertical.1.y > 50.0);
}

#[test]
fn numbers_that_cannot_make_a_flange_are_refused_saying_which() {
    let cases: Vec<(WeldNeckFlange, &str)> = vec![
        (
            WeldNeckFlange {
                bore: -1.0,
                ..sample()
            },
            "bore",
        ),
        (
            WeldNeckFlange {
                outside_diameter: f64::NAN,
                ..sample()
            },
            "outside_diameter",
        ),
        (
            WeldNeckFlange {
                bolt_count: 0,
                ..sample()
            },
            "bolt_count",
        ),
        (
            WeldNeckFlange {
                bolt_count: 65,
                ..sample()
            },
            "bolt_count",
        ),
        (
            WeldNeckFlange {
                bore: 45.0,
                ..sample()
            },
            "nest",
        ),
        (
            WeldNeckFlange {
                hub_diameter_end: 60.0,
                ..sample()
            },
            "nest",
        ),
        (
            WeldNeckFlange {
                hub_diameter_base: 120.0,
                ..sample()
            },
            "nest",
        ),
        (
            WeldNeckFlange {
                raised_face_diameter: 20.0,
                ..sample()
            },
            "raised_face_diameter",
        ),
        (
            WeldNeckFlange {
                overall_length: 16.0,
                ..sample()
            },
            "overall_length",
        ),
        (
            WeldNeckFlange {
                bolt_circle_diameter: 55.0,
                ..sample()
            },
            "clear the hub",
        ),
        (
            WeldNeckFlange {
                bolt_circle_diameter: 92.0,
                ..sample()
            },
            "off the edge",
        ),
        (
            WeldNeckFlange {
                raised_face_diameter: 68.0,
                ..sample()
            },
            "inside the bolt holes",
        ),
        (
            WeldNeckFlange {
                raised_face_height: -1.0,
                ..sample()
            },
            "raised_face_height",
        ),
    ];
    for (flange, mentions) in cases {
        let error = flange.shapes().expect_err(mentions);
        assert!(error.0.contains(mentions), "{mentions}: {error}");
    }
}

#[test]
fn a_shape_can_be_moved() {
    let moved = Shape::Circle {
        center: Point::new(1.0, 2.0),
        radius: 3.0,
    }
    .moved(10.0, -2.0);
    assert_eq!(
        moved,
        Shape::Circle {
            center: Point::new(11.0, 0.0),
            radius: 3.0
        }
    );
    let line = Shape::Line(Point::new(0.0, 0.0), Point::new(1.0, 1.0)).moved(1.0, 1.0);
    assert_eq!(
        line,
        Shape::Line(Point::new(1.0, 1.0), Point::new(2.0, 2.0))
    );
}
