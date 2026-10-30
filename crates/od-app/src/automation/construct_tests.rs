//! `entities.construct` through the real dispatcher: each step does what a
//! drafter's command does, to the number, as one undo step by the caller.

use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

pub(super) const INSTANCE: &str = "inst1";
pub(super) const CALLER: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

pub(super) fn call(
    state: &mut AppState,
    op: &str,
    extra: Value,
) -> Result<Value, od_api::ApiError> {
    let mut args = json!({
        "document_id": document_id(INSTANCE, state.docs[state.active].uid),
        "expected_revision": state.session().revision().to_string(),
    });
    for (key, value) in extra.as_object().into_iter().flatten() {
        args[key] = value.clone();
    }
    dispatch_test(state, INSTANCE, &CALLER, op, args)
}

pub(super) fn construct(state: &mut AppState, steps: Value) -> Result<Value, od_api::ApiError> {
    call(state, "entities.construct", json!({"steps": steps}))
}

/// Add shapes through the batch tool; their ids by name.
pub(super) fn draw(state: &mut AppState, operations: Value) -> Value {
    call(
        state,
        "entities.apply_batch",
        json!({"operations": operations}),
    )
    .unwrap()["ids"]
        .clone()
}

pub(super) fn circle(name: &str, x: f64, y: f64, r: f64) -> Value {
    json!({"op": "add", "client_ref": name, "entity": {"geometry": {
        "type": "circle", "center": {"x": x, "y": y}, "radius": r}}})
}

fn line(name: &str, a: (f64, f64), b: (f64, f64)) -> Value {
    json!({"op": "add", "client_ref": name, "entity": {"geometry": {
        "type": "line", "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1}}}})
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

pub(super) fn entities(state: &AppState) -> Vec<Entity> {
    state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.entity.clone())
        .collect()
}

pub(super) fn circles(state: &AppState) -> Vec<(Point, f64)> {
    entities(state)
        .into_iter()
        .filter_map(|e| match e {
            Entity::Circle { center, radius } => Some((center, radius)),
            _ => None,
        })
        .collect()
}

pub(super) fn id(ids: &Value, name: &str) -> Value {
    ids[name].clone()
}

fn polygon(state: &mut AppState, extra: Value) -> Vec<Point> {
    let mut step = json!({"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0},
                          "sides": 6, "radius": 10});
    for (key, value) in extra.as_object().into_iter().flatten() {
        step[key] = value.clone();
    }
    construct(state, json!([step])).unwrap();
    match entities(state).remove(0) {
        Entity::Polyline { points, closed, .. } => {
            assert!(closed);
            points
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_hexagon_has_its_corners_on_the_radius_and_its_bottom_side_level() {
    let mut state = AppState::new();
    let points = polygon(&mut state, json!({}));
    assert_eq!(points.len(), 6);
    for p in &points {
        assert!(near(p.x.hypot(p.y), 10.0));
    }
    let lowest = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert_eq!(points.iter().filter(|p| near(p.y, lowest)).count(), 2);
}

#[test]
fn a_circumscribed_polygon_touches_its_circle_at_the_middle_of_each_side() {
    let mut state = AppState::new();
    let points = polygon(&mut state, json!({"fit": "circumscribed"}));
    for pair in points.iter().zip(points.iter().cycle().skip(1)) {
        let middle = Point::new((pair.0.x + pair.1.x) / 2.0, (pair.0.y + pair.1.y) / 2.0);
        assert!(near(middle.x.hypot(middle.y), 10.0));
    }
}

#[test]
fn a_rotation_turns_the_polygon_about_its_centre() {
    let mut state = AppState::new();
    let turned = polygon(&mut state, json!({"rotation": std::f64::consts::FRAC_PI_2}));
    let plain = {
        let mut other = AppState::new();
        polygon(&mut other, json!({}))
    };
    for p in &plain {
        let want = Point::new(-p.y, p.x);
        assert!(turned
            .iter()
            .any(|t| near(t.x, want.x) && near(t.y, want.y)));
    }
}

#[test]
fn a_polar_array_makes_the_holes_of_a_bolt_circle_from_one() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("hole", 37.5, 0.0, 5.0)]));
    let result = construct(
        &mut state,
        json!([{"op": "polar_array", "targets": [id(&ids, "hole")],
                "center": {"x": 0, "y": 0}, "count": 8, "ref_prefix": "h"}]),
    )
    .unwrap();
    let all = circles(&state);
    assert_eq!(all.len(), 8);
    let mut angles: Vec<f64> = all
        .iter()
        .map(|(c, r)| {
            assert!(near(*r, 5.0));
            assert!(near(c.x.hypot(c.y), 37.5), "on the bolt circle");
            c.y.atan2(c.x).rem_euclid(std::f64::consts::TAU)
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    for pair in angles.windows(2) {
        assert!(near(pair[1] - pair[0], std::f64::consts::TAU / 8.0));
    }
    assert!(result["ids"]["h_1"].is_string() && result["ids"]["h_7"].is_string());
    assert!(
        result["ids"].get("h_8").is_none(),
        "seven copies and the original"
    );
}

#[test]
fn a_polar_array_over_part_of_a_turn_spans_exactly_that_angle() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 10.0, 0.0, 1.0)]));
    construct(
        &mut state,
        json!([{"op": "polar_array", "targets": [id(&ids, "c")], "center": {"x": 0, "y": 0},
                "count": 3, "fill_angle": std::f64::consts::PI, "ref_prefix": "a"}]),
    )
    .unwrap();
    let last = circles(&state)
        .into_iter()
        .find(|(c, _)| c.y.abs() < 1e-9 && c.x < 0.0)
        .expect("the third item is half a turn away");
    assert!(near(last.0.x, -10.0));
}

#[test]
fn a_rectangular_array_places_copies_on_a_grid() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 0.0, 0.0, 1.0)]));
    construct(
        &mut state,
        json!([{"op": "rect_array", "targets": [id(&ids, "c")], "columns": 3, "rows": 2,
                "column_spacing": 10, "row_spacing": -5, "ref_prefix": "g"}]),
    )
    .unwrap();
    let mut spots: Vec<(i64, i64)> = circles(&state)
        .iter()
        .map(|(c, _)| (c.x.round() as i64, c.y.round() as i64))
        .collect();
    spots.sort_unstable();
    assert_eq!(
        spots,
        [(0, -5), (0, 0), (10, -5), (10, 0), (20, -5), (20, 0)]
    );
}

#[test]
fn a_mirror_with_a_copy_keeps_the_original_and_without_one_changes_it() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([line("l", (1.0, 1.0), (3.0, 2.0))]));
    let axis = json!({"axis_start": {"x": 0, "y": -1}, "axis_end": {"x": 0, "y": 1}});
    let mut step = json!({"op": "mirror", "targets": [id(&ids, "l")], "copy_as": "m"});
    for (k, v) in axis.as_object().unwrap() {
        step[k] = v.clone();
    }
    construct(&mut state, json!([step])).unwrap();
    let lines: Vec<(Point, Point)> = entities(&state)
        .into_iter()
        .filter_map(|e| match e {
            Entity::Line { start, end } => Some((start, end)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines
        .iter()
        .any(|(a, b)| near(a.x, -1.0) && near(b.x, -3.0) && near(b.y, 2.0)));
    // In place now: the original goes across the axis too.
    let mut again = json!({"op": "mirror", "targets": [id(&ids, "l")]});
    for (k, v) in axis.as_object().unwrap() {
        again[k] = v.clone();
    }
    construct(&mut state, json!([again])).unwrap();
    let flipped = entities(&state)
        .into_iter()
        .filter(|e| matches!(e, Entity::Line { start, .. } if start.x < 0.0))
        .count();
    assert_eq!(flipped, 2);
}

#[test]
fn move_rotate_and_scale_change_a_shape_in_place_exactly() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 10.0, 0.0, 2.0)]));
    let target = json!([id(&ids, "c")]);
    construct(
        &mut state,
        json!([
            {"op": "move", "targets": target, "dx": 5, "dy": 1},
            {"op": "rotate", "targets": target, "center": {"x": 0, "y": 0}, "angle": std::f64::consts::PI},
            {"op": "scale", "targets": target, "base": {"x": 0, "y": 0}, "factor": 2},
        ]),
    )
    .unwrap();
    let (c, r) = circles(&state)[0];
    // (10,0) -> (15,1) -> (-15,-1) -> (-30,-2); radius 2 -> 4.
    assert!(
        near(c.x, -30.0) && near(c.y, -2.0) && near(r, 4.0),
        "{c:?} {r}"
    );
}

#[test]
fn an_offset_makes_a_parallel_copy_on_the_side_given() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 0.0, 0.0, 10.0)]));
    construct(
        &mut state,
        json!([{"op": "offset", "target": id(&ids, "c"), "distance": 3,
                "toward": {"x": 0, "y": 0}, "client_ref": "inner"}]),
    )
    .unwrap();
    let mut radii: Vec<f64> = circles(&state).iter().map(|(_, r)| *r).collect();
    radii.sort_by(f64::total_cmp);
    assert!(near(radii[0], 7.0) && near(radii[1], 10.0));
}

#[test]
fn an_offset_that_does_not_fit_is_refused() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 0.0, 0.0, 2.0)]));
    let error = construct(
        &mut state,
        json!([{"op": "offset", "target": id(&ids, "c"), "distance": 5,
                "toward": {"x": 0, "y": 0}, "client_ref": "x"}]),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidGeometry);
}

#[test]
fn a_step_can_use_what_an_earlier_step_made() {
    let mut state = AppState::new();
    construct(
        &mut state,
        json!([
            {"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 4, "radius": 10},
            {"op": "move", "targets": ["p"], "dx": 100, "dy": 0, "copy_as": "far"},
        ]),
    )
    .unwrap();
    assert_eq!(entities(&state).len(), 2);
}

#[test]
fn a_tangent_arc_bends_off_the_end_of_a_line_and_its_walls_follow() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([line("pipe", (0.0, 0.0), (2000.0, 0.0))]));
    let pipe = ids["pipe"].as_str().unwrap().to_owned();
    construct(
        &mut state,
        json!([
            {"op": "tangent_arc", "target": pipe, "at": "end", "radius": 229.0,
             "sweep": std::f64::consts::FRAC_PI_2, "client_ref": "bend"},
            {"op": "offset", "target": "bend", "distance": 84.15,
             "toward": {"x": 2000.0, "y": 229.0}, "client_ref": "inner"},
            {"op": "offset", "target": "bend", "distance": 84.15,
             "toward": {"x": 2400.0, "y": -400.0}, "client_ref": "outer"},
        ]),
    )
    .unwrap();
    let arcs: Vec<(Point, f64)> = entities(&state)
        .into_iter()
        .filter_map(|e| match e {
            Entity::Arc { center, radius, .. } => Some((center, radius)),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 3);
    for (c, _) in &arcs {
        assert!(near(c.x, 2000.0) && near(c.y, 229.0), "{c:?}");
    }
    let mut radii: Vec<f64> = arcs.iter().map(|a| a.1).collect();
    radii.sort_by(f64::total_cmp);
    assert!(near(radii[0], 144.85) && near(radii[1], 229.0) && near(radii[2], 313.15));
}

#[test]
fn a_tangent_arc_off_a_circle_or_with_no_sweep_is_refused() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 0.0, 0.0, 5.0)]));
    let target = ids["c"].as_str().unwrap().to_owned();
    for sweep in [1.0, 0.0] {
        let step = json!([{"op": "tangent_arc", "target": target, "at": "end", "radius": 5.0,
                           "sweep": sweep, "client_ref": "b"}]);
        let error = construct(&mut state, step).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidGeometry);
    }
}

#[test]
fn a_whole_pipe_bend_is_one_request_with_its_walls_on_their_own_layer() {
    let mut state = AppState::new();
    let layers = call(
        &mut state,
        "layers.apply_batch",
        json!({"operations": [{"op": "create", "client_ref": "w", "name": "Walls"}]}),
    )
    .unwrap();
    let walls = layers["ids"]["w"].clone();
    construct(
        &mut state,
        json!([
            {"op": "line", "client_ref": "axis", "start": {"x": 0, "y": 0}, "end": {"x": 2000, "y": 0}},
            {"op": "tangent_arc", "target": "axis", "at": "end", "radius": 229.0,
             "sweep": std::f64::consts::FRAC_PI_2, "client_ref": "bend"},
            {"op": "offset", "target": "axis", "distance": 84.15, "toward": {"x": 100, "y": 50},
             "client_ref": "wall_a", "layer_id": walls},
            {"op": "offset", "target": "bend", "distance": 84.15, "toward": {"x": 2000, "y": 229},
             "client_ref": "wall_b", "layer_id": walls},
        ]),
    )
    .unwrap();
    let objects = state.session().document().objects().to_vec();
    assert_eq!(objects.len(), 4);
    let on_walls = objects
        .iter()
        .filter(|o| o.attrs.layer.0.to_string() == walls.as_str().unwrap())
        .count();
    assert_eq!(on_walls, 2, "only the two offset walls moved layer");
}

#[test]
fn a_line_needs_two_different_points() {
    let mut state = AppState::new();
    let step = json!([{"op": "line", "client_ref": "a",
                       "start": {"x": 1, "y": 1}, "end": {"x": 1, "y": 1}}]);
    let error = construct(&mut state, step).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidGeometry);
}

#[test]
fn a_tangent_arc_can_leave_the_start_of_a_line() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([line("pipe", (0.0, 0.0), (10.0, 0.0))]));
    let pipe = ids["pipe"].as_str().unwrap().to_owned();
    construct(
        &mut state,
        json!([{"op": "tangent_arc", "target": pipe, "at": "start", "radius": 5.0,
                "sweep": std::f64::consts::FRAC_PI_2, "client_ref": "b"}]),
    )
    .unwrap();
    // Heading left from the origin and turning left: the centre is below.
    let centre = entities(&state).into_iter().find_map(|e| match e {
        Entity::Arc { center, .. } => Some(center),
        _ => None,
    });
    let c = centre.unwrap();
    assert!(near(c.x, 0.0) && near(c.y, -5.0), "{c:?}");
}
