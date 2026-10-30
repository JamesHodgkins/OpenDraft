//! `calculate.evaluate` through the real dispatcher: numbers come back exact,
//! a mistake is refused with its step, and the drawing is untouched.

use od_api::ErrorCode;
use serde_json::{json, Value};

use super::{dispatch_test, Caller};
use crate::state::AppState;

const CALLER: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn calculate(state: &mut AppState, steps: Value) -> Result<Value, od_api::ApiError> {
    dispatch_test(
        state,
        "inst1",
        &CALLER,
        "calculate.evaluate",
        json!({"steps": steps}),
    )
}

#[test]
fn a_bolt_circle_position_comes_back_exact_with_no_drawing_named() {
    let mut state = AppState::new();
    let result = calculate(
        &mut state,
        json!([
            {"name": "pcd", "expression": "75"},
            {"name": "angle", "expression": "rad(360/8/2 + 90)"},
            {"name": "x", "expression": "pcd/2*cos(angle)"},
            {"name": "y", "expression": "pcd/2*sin(angle)"},
        ]),
    )
    .unwrap();
    let value = |name: &str| {
        result["values"]
            .as_array()
            .and_then(|v| v.iter().find(|e| e["name"] == name))
            .and_then(|e| e["value"].as_f64())
            .unwrap_or(f64::NAN)
    };
    let angle = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU / 16.0;
    assert!((value("x") - 37.5 * angle.cos()).abs() < 1e-12);
    assert!((value("y") - 37.5 * angle.sin()).abs() < 1e-12);
    assert_eq!(result["values"][0]["name"], "pcd", "in the order given");
}

#[test]
fn a_mistake_is_refused_with_the_step_and_the_reason() {
    let mut state = AppState::new();
    let error = calculate(
        &mut state,
        json!([{"name": "a", "expression": "1"}, {"name": "b", "expression": "a/0"}]),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    assert!(error.message.contains("step 2 (b)") && error.message.contains("division by zero"));
}

#[test]
fn the_drawing_is_not_touched() {
    let mut state = AppState::new();
    let before = state.session().revision();
    calculate(&mut state, json!([{"name": "a", "expression": "2*pi"}])).unwrap();
    assert_eq!(state.session().revision(), before);
    assert!(state.session().history().top_undo().is_none());
}

#[test]
fn an_unknown_field_is_an_argument_error() {
    let mut state = AppState::new();
    let error = dispatch_test(
        &mut state,
        "inst1",
        &CALLER,
        "calculate.evaluate",
        json!({"steps": [{"name": "a", "expression": "1", "unit": "mm"}]}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}
