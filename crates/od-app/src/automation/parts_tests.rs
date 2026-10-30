//! `parts.draw` through the real dispatcher: a part is drawn as one undoable
//! step by the caller, and numbers that cannot make it change nothing.

use od_api::ErrorCode;
use od_commands::Provenance;
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const CALLER: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn flange(state: &AppState, change: impl FnOnce(&mut Value)) -> Value {
    let mut args = json!({
        "document_id": document_id(INSTANCE, state.docs[state.active].uid),
        "expected_revision": state.session().revision().to_string(),
        "part": {
            "kind": "weld_neck_flange",
            "outside_diameter": 100, "thickness": 14,
            "raised_face_diameter": 60, "raised_face_height": 2,
            "hub_diameter_base": 50, "hub_diameter_end": 38,
            "overall_length": 60, "bore": 30,
            "bolt_circle_diameter": 75, "bolt_count": 4, "bolt_hole_diameter": 10
        }
    });
    change(&mut args);
    args
}

fn draw(state: &mut AppState, args: Value) -> Result<Value, od_api::ApiError> {
    dispatch_test(state, INSTANCE, &CALLER, "parts.draw", args)
}

#[test]
fn a_flange_is_drawn_as_one_undoable_step_by_the_caller() {
    let mut state = AppState::new();
    let args = flange(&state, |_| {});
    let result = draw(&mut state, args).unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(result["history_step"], "Weld neck flange");
    // Two section halves, eight circles, two centrelines.
    assert_eq!(state.session().document().len(), 12);
    let top = state.session().history().top_undo().unwrap();
    assert_eq!(top.provenance, Provenance::Automation(7));
    state.docs[0].session.undo();
    assert!(
        state.session().document().is_empty(),
        "one undo takes it all"
    );
}

#[test]
fn the_origin_places_the_part() {
    let mut state = AppState::new();
    let args = flange(&state, |a| a["origin"] = json!({"x": 1000.0, "y": 500.0}));
    draw(&mut state, args).unwrap();
    let (mut lowest, mut highest) = (f64::MAX, f64::MIN);
    for object in state.session().document().objects() {
        if let od_core::Entity::Line { start, end } = &object.entity {
            lowest = lowest.min(start.y).min(end.y);
            highest = highest.max(start.y).max(end.y);
        }
    }
    // The vertical centreline runs 60 either side of the axis at y = 500.
    assert!((lowest - 440.0).abs() < 1e-9 && (highest - 560.0).abs() < 1e-9);
}

#[test]
fn numbers_that_cannot_make_a_flange_draw_nothing_and_say_why() {
    let mut state = AppState::new();
    let args = flange(&state, |a| a["part"]["bore"] = json!(45));
    let error = draw(&mut state, args).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidGeometry);
    assert!(error.message.contains("nest"), "{}", error.message);
    assert!(state.session().document().is_empty());
    assert!(state.session().history().top_undo().is_none());
}

#[test]
fn a_stale_revision_is_refused_like_any_batch() {
    let mut state = AppState::new();
    let args = flange(&state, |a| a["expected_revision"] = json!("99"));
    assert_eq!(
        draw(&mut state, args).unwrap_err().code,
        ErrorCode::StaleRevision
    );
    assert!(state.session().document().is_empty());
}

#[test]
fn a_retry_with_the_same_key_does_not_draw_twice() {
    let mut state = AppState::new();
    let args = flange(&state, |a| a["idempotency_key"] = json!("flange-1"));
    draw(&mut state, args.clone()).unwrap();
    let again = draw(&mut state, args).unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(state.session().document().len(), 12);
}

#[test]
fn an_unknown_kind_or_field_is_an_argument_error() {
    let mut state = AppState::new();
    let args = flange(&state, |a| a["part"]["kind"] = json!("teapot"));
    assert_eq!(
        draw(&mut state, args).unwrap_err().code,
        ErrorCode::InvalidArguments
    );
    let args = flange(&state, |a| a["part"]["colour"] = json!("red"));
    assert_eq!(
        draw(&mut state, args).unwrap_err().code,
        ErrorCode::InvalidArguments
    );
}

/// For looking at a part: `OD_PARTS_PNG=C:\temp\flange.png cargo test -p od-app
/// --bin opendraft look_at_a_flange -- --ignored`.
#[test]
#[ignore = "writes a picture of a flange to OD_PARTS_PNG, for looking at"]
fn look_at_a_flange() {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let Some(path) = std::env::var_os("OD_PARTS_PNG") else {
        panic!("set OD_PARTS_PNG to where the picture should go");
    };
    let mut state = AppState::new();
    let args = flange(&state, |_| {});
    draw(&mut state, args).unwrap();
    let document = document_id(INSTANCE, state.docs[0].uid);
    let picture = dispatch_test(
        &mut state,
        INSTANCE,
        &CALLER,
        "view.render",
        json!({
            "document_id": document,
            "format": "png",
            "width": 1000,
        }),
    )
    .unwrap();
    let bytes = STANDARD
        .decode(picture["png_base64"].as_str().unwrap())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}
