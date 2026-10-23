use od_api::ErrorCode;
use od_core::{Entity, MText, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const ME: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
    Entity::Line {
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
    }
}

fn call(state: &mut AppState, op: &str, mut args: Value) -> Result<Value, od_api::ApiError> {
    args["document_id"] = json!(document_id(INSTANCE, state.docs[state.active].uid));
    dispatch_test(state, INSTANCE, &ME, op, args)
}

fn ids(state: &AppState) -> Vec<String> {
    state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id.0.to_string())
        .collect()
}

#[test]
fn measure_answers_in_request_order_with_null_for_unknown_ids() {
    let mut state = AppState::new();
    state.add_entity(line((0.0, 0.0), (3.0, 4.0)));
    state.add_entity(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
    });
    let all = ids(&state);
    let result = call(
        &mut state,
        "geometry.measure",
        json!({"entity_ids": [all[1], "999", all[0]]}),
    )
    .unwrap();
    let m = result["measurements"].as_array().unwrap();
    assert_eq!(m[0]["kind"], "circle");
    assert!((m[0]["area"].as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-12);
    assert!(m[1].is_null());
    assert_eq!(m[2]["length"], 5.0);
    assert_eq!(m[2]["exactness"], "exact");
    let many: Vec<String> = (0..1001).map(|i| i.to_string()).collect();
    let error = call(&mut state, "geometry.measure", json!({"entity_ids": many})).unwrap_err();
    assert_eq!(error.code, ErrorCode::PayloadTooLarge);
}

#[test]
fn crossings_are_found_and_unsupported_kinds_are_named_not_skipped_silently() {
    let mut state = AppState::new();
    state.add_entity(line((0.0, 0.0), (10.0, 10.0)));
    state.add_entity(line((0.0, 10.0), (10.0, 0.0)));
    state.add_entity(Entity::Circle {
        center: Point::new(5.0, 5.0),
        radius: 2.0,
    });
    state.add_entity(Entity::MText(Box::new(MText::new(
        Point::new(5.0, 5.0),
        2.5,
        "Hi",
        od_core::TextStyleId(0),
    ))));
    let all = ids(&state);
    let result = call(
        &mut state,
        "geometry.intersections",
        json!({"entity_ids": all}),
    )
    .unwrap();
    let points = result["points"].as_array().unwrap();
    // The lines cross once and the circle crosses each line twice.
    assert_eq!(points.len(), 5, "{result}");
    let crossing = points
        .iter()
        .find(|p| p["entity_a"] == all[0] && p["entity_b"] == all[1])
        .unwrap();
    assert!((crossing["point"]["x"].as_f64().unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(result["unsupported"], json!([all[3]]));
    assert_eq!(result["pairs_tested"], 3);
}

#[test]
fn intersections_need_two_known_entities_and_respect_the_pair_limit() {
    let mut state = AppState::new();
    state.add_entity(line((0.0, 0.0), (1.0, 1.0)));
    let one = ids(&state);
    let error = call(
        &mut state,
        "geometry.intersections",
        json!({"entity_ids": [one[0], one[0]]}),
    );
    assert_eq!(error.unwrap_err().code, ErrorCode::InvalidArguments);
    let error = call(
        &mut state,
        "geometry.intersections",
        json!({"entity_ids": [one[0], "77"]}),
    );
    assert_eq!(error.unwrap_err().code, ErrorCode::EntityNotFound);

    // 200 lines through one spot: 19,900 overlapping pairs, over the cap.
    let mut state = AppState::new();
    for i in 0..200 {
        let t = f64::from(i);
        state.add_entity(line((-100.0, -100.0 + t), (100.0, 100.0 - t)));
    }
    let all = ids(&state);
    let error = call(
        &mut state,
        "geometry.intersections",
        json!({"entity_ids": all}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_eq!(error.details["limit"], "10000");
    let too_many: Vec<String> = (0..201).map(|i| i.to_string()).collect();
    let error = call(
        &mut state,
        "geometry.intersections",
        json!({"entity_ids": too_many}),
    );
    assert_eq!(error.unwrap_err().code, ErrorCode::PayloadTooLarge);
}

fn drawing() -> AppState {
    let mut state = AppState::new();
    state.add_entity(line((0.0, 0.0), (100.0, 0.0)));
    state.add_entity(Entity::Circle {
        center: Point::new(50.0, 50.0),
        radius: 20.0,
    });
    state.add_entity(line((500.0, 500.0), (600.0, 500.0)));
    state
}

#[test]
fn a_render_is_an_svg_labelled_with_its_revision_and_window() {
    let mut state = drawing();
    let result = call(
        &mut state,
        "view.render",
        json!({"bounds": {"min_x": -10.0, "min_y": -10.0, "max_x": 110.0, "max_y": 90.0}, "width": 400.0}),
    )
    .unwrap();
    assert_eq!(result["revision"], "3");
    assert_eq!(result["width"], 400.0);
    assert!((result["height"].as_f64().unwrap() - 1000.0 / 3.0).abs() < 1e-9);
    let svg = result["svg"].as_str().unwrap();
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("<polyline") || svg.contains("<polygon"));
    assert!(result["fidelity"]
        .as_str()
        .unwrap()
        .contains("plotting pipeline"));
    assert_eq!(result["region"]["min"]["x"], -10.0);
    // The far line is outside the window: line and circle only.
    assert_eq!(
        svg.matches("<polyline").count() + svg.matches("<polygon").count(),
        2
    );
}

#[test]
fn a_render_of_chosen_entities_frames_them_and_the_default_frames_everything() {
    let mut state = drawing();
    let all = ids(&state);
    let one = call(&mut state, "view.render", json!({"entity_ids": [all[1]]})).unwrap();
    let region = &one["region"];
    assert!(
        region["min"]["x"].as_f64().unwrap() < 30.0 && region["max"]["x"].as_f64().unwrap() > 70.0
    );
    assert!(
        region["max"]["x"].as_f64().unwrap() < 100.0,
        "framed to the circle, not the drawing"
    );
    let whole = call(&mut state, "view.render", json!({})).unwrap();
    assert!(whole["region"]["max"]["x"].as_f64().unwrap() >= 600.0);
}

#[test]
fn bad_render_requests_are_refused_clearly() {
    let mut state = drawing();
    let code =
        |state: &mut AppState, args: Value| call(state, "view.render", args).unwrap_err().code;
    assert_eq!(
        code(&mut state, json!({"width": 10.0})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"width": 100000.0})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(
            &mut state,
            json!({"bounds": {"min_x": 5.0, "min_y": 0.0, "max_x": 5.0, "max_y": 1.0}})
        ),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"entity_ids": ["999"]})),
        ErrorCode::EntityNotFound
    );
    assert_eq!(
        code(&mut state, json!({"expected_revision": "0"})),
        ErrorCode::StaleRevision
    );
    assert_eq!(
        code(
            &mut state,
            json!({"space": {"kind": "paper", "layout_id": "9"}})
        ),
        ErrorCode::InvalidReference
    );
    let mut empty = AppState::new();
    assert_eq!(code(&mut empty, json!({})), ErrorCode::InvalidArguments);
}

#[test]
fn a_picture_that_would_be_too_big_is_refused_not_truncated() {
    let mut state = AppState::new();
    let points: Vec<Point> = (0..520_000)
        .map(|i| Point::new(f64::from(i), f64::from(i % 7)))
        .collect();
    state.add_entity(Entity::Polyline {
        points,
        closed: false,
        bulges: vec![],
        widths: vec![],
    });
    let error = call(&mut state, "view.render", json!({})).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_eq!(error.details["limit"], "500000");
}

#[test]
fn the_read_tools_work_for_a_reader_without_write_access() {
    // They need only drawing.read: the scope table says so.
    for op in ["geometry.measure", "geometry.intersections", "view.render"] {
        assert_eq!(
            od_api::ops::required_scope(op),
            Some(od_api::Scope::DrawingRead)
        );
    }
}
