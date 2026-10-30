//! The point of `entities.construct`: a part is composed from generic steps,
//! with no recipe for that part. Here a flange-like turned part is drawn the
//! way a drafter draws it: half the section as one outline, mirrored for the
//! other half, both hatched; the circles of the face view; one bolt hole,
//! polar-arrayed round its circle. The numbers are made up, and every step is
//! generic, so the same steps draw any part of this kind.

use od_core::Entity;
use serde_json::{json, Value};

use super::construct_tests::{call, circles, entities};
use crate::state::AppState;

/// The upper half of the section, in (length, radius) pairs: a raised face,
/// a thick flange and a tapered hub.
const PROFILE: [(f64, f64); 8] = [
    (0.0, 15.0),
    (0.0, 30.0),
    (2.0, 30.0),
    (2.0, 50.0),
    (16.0, 50.0),
    (16.0, 25.0),
    (60.0, 19.0),
    (60.0, 15.0),
];

/// The face view's centre, and the bolt circle's radius.
const FACE_X: f64 = -75.0;
const BOLT_RADIUS: f64 = 37.5;

fn circle(name: &str, radius: f64) -> Value {
    json!({"op": "add", "client_ref": name, "entity": {"geometry": {
        "type": "circle", "center": {"x": FACE_X, "y": 0}, "radius": radius}}})
}

fn line(name: &str, a: (f64, f64), b: (f64, f64)) -> Value {
    json!({"op": "add", "client_ref": name, "entity": {"geometry": {
        "type": "line", "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1}}}})
}

/// Draw the whole part from generic steps.
fn compose(state: &mut AppState) {
    // A bolt hole straddling the vertical, half a step off it.
    let step = std::f64::consts::TAU / 8.0;
    let at = std::f64::consts::FRAC_PI_2 + step / 2.0;
    let hole = json!({"op": "add", "client_ref": "hole", "entity": {"geometry": {
        "type": "circle", "radius": 5,
        "center": {"x": FACE_X + BOLT_RADIUS * at.cos(), "y": BOLT_RADIUS * at.sin()}}}});
    let outline: Vec<Value> = PROFILE
        .iter()
        .map(|(x, r)| json!({"x": x, "y": r}))
        .collect();
    let drawn = call(
        state,
        "entities.apply_batch",
        json!({"operations": [
            {"op": "add", "client_ref": "half", "entity": {"geometry": {
                "type": "polyline", "closed": true, "points": outline}}},
            circle("edge", 50.0),
            circle("raised", 30.0),
            circle("bore", 15.0),
            circle("bolt_circle", BOLT_RADIUS),
            hole,
            line("axis", (-135.0, 0.0), (65.0, 0.0)),
            line("vertical", (FACE_X, -55.0), (FACE_X, 55.0)),
        ]}),
    )
    .unwrap();
    // The other half, and the rest of the bolt holes, from what is there.
    let made = call(
        state,
        "entities.construct",
        json!({"steps": [
            {"op": "mirror", "targets": [drawn["ids"]["half"]], "copy_as": "other",
             "axis_start": {"x": 0, "y": 0}, "axis_end": {"x": 1, "y": 0}},
            {"op": "polar_array", "targets": [drawn["ids"]["hole"]],
             "center": {"x": FACE_X, "y": 0}, "count": 8, "ref_prefix": "h"},
        ]}),
    )
    .unwrap();
    // Both halves hatched together.
    call(
        state,
        "entities.apply_batch",
        json!({"operations": [{"op": "add", "client_ref": "hatch", "entity": {"geometry": {
            "type": "hatch", "boundary": [{"entity_id": drawn["ids"]["half"]},
                         {"entity_id": made["ids"]["other_1"]}],
            "pattern": {"type": "named", "name": "ANSI31", "scale": 14},
            "associative": true}}}]}),
    )
    .unwrap();
}

#[test]
fn a_turned_part_is_composed_from_generic_steps() {
    let mut state = AppState::new();
    compose(&mut state);
    let all = entities(&state);
    let count = |keep: fn(&Entity) -> bool| all.iter().filter(|e| keep(e)).count();
    assert_eq!(
        count(|e| matches!(e, Entity::Polyline { points, .. } if points.len() == 8)),
        2
    );
    assert_eq!(count(|e| matches!(e, Entity::Hatch(_))), 1);
    assert_eq!(count(|e| matches!(e, Entity::Line { .. })), 2);
    // Edge, raised face, bore, bolt circle, and eight holes.
    assert_eq!(circles(&state).len(), 12);
}

#[test]
fn the_mirrored_half_is_the_exact_reflection_of_the_first() {
    let mut state = AppState::new();
    compose(&mut state);
    let halves: Vec<Vec<od_core::Point>> = entities(&state)
        .into_iter()
        .filter_map(|e| match e {
            Entity::Polyline { points, .. } if points.len() == 8 => Some(points),
            _ => None,
        })
        .collect();
    for (a, b) in halves[0].iter().zip(&halves[1]) {
        assert!((a.x - b.x).abs() < 1e-9 && (a.y + b.y).abs() < 1e-9);
    }
}

#[test]
fn the_holes_are_equally_spaced_on_the_bolt_circle_and_none_is_on_a_centreline() {
    let mut state = AppState::new();
    compose(&mut state);
    let holes: Vec<od_core::Point> = circles(&state)
        .into_iter()
        .filter(|(_, r)| (r - 5.0).abs() < 1e-9)
        .map(|(c, _)| c)
        .collect();
    assert_eq!(holes.len(), 8);
    for h in &holes {
        assert!(((h.x - FACE_X).hypot(h.y) - BOLT_RADIUS).abs() < 1e-9);
        assert!((h.x - FACE_X).abs() > 1e-6 && h.y.abs() > 1e-6);
    }
}

#[test]
fn the_hatch_fills_both_halves() {
    let mut state = AppState::new();
    compose(&mut state);
    let hatch = state
        .session()
        .document()
        .objects()
        .iter()
        .find_map(|o| match &o.entity {
            Entity::Hatch(h) => Some((**h).clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(hatch.loops.len(), 2);
}

/// For looking at what was composed: `OD_COMPOSE_PNG=C:\temp\part.png cargo
/// test -p od-app --bin opendraft look_at_a_composed_part -- --ignored`.
#[test]
#[ignore = "writes a picture to OD_COMPOSE_PNG, for looking at"]
fn look_at_a_composed_part() {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let Some(path) = std::env::var_os("OD_COMPOSE_PNG") else {
        panic!("set OD_COMPOSE_PNG to where the picture should go");
    };
    let mut state = AppState::new();
    compose(&mut state);
    let picture = call(
        &mut state,
        "view.render",
        json!({"format": "png", "width": 1000}),
    )
    .unwrap();
    let bytes = STANDARD
        .decode(picture["png_base64"].as_str().unwrap())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}
