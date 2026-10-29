//! `drawing.summary`: the overview a caller uses to decide where to look.

use od_api::ErrorCode;
use od_core::mtext::MText;
use od_core::text_style::TextStyleId;
use od_core::{Entity, Insert, Point};
use serde_json::{json, Value};

use super::{document_id, handle_read};
use crate::state::AppState;

const INSTANCE: &str = "inst1";

fn line(x: f64, y: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, y),
        end: Point::new(x + 1.0, y),
    }
}

fn summary(state: &AppState, extra: Value) -> Result<Value, od_api::ApiError> {
    let mut args = json!({"document_id": document_id(INSTANCE, state.docs[state.active].uid)});
    for (key, value) in extra.as_object().into_iter().flatten() {
        args[key] = value.clone();
    }
    handle_read(state, INSTANCE, "drawing.summary", args)
}

fn text(contents: &str, at: Point) -> Entity {
    Entity::MText(Box::new(MText::new(
        at,
        2.5,
        contents,
        TextStyleId::STANDARD,
    )))
}

#[test]
fn it_counts_by_kind_and_layer_and_boxes_what_there_is() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 0.0));
    state.add_entity(line(10.0, 0.0));
    state.add_entity(Entity::Circle {
        center: Point::new(5.0, 5.0),
        radius: 1.0,
    });
    let result = summary(&state, json!({})).unwrap();
    assert_eq!(result["scanned"], 3);
    assert_eq!(
        result["by_kind"],
        json!([{"kind": "line", "count": 2}, {"kind": "circle", "count": 1}])
    );
    assert_eq!(result["layers_total"], 1);
    assert_eq!(result["by_layer"][0]["name"], "0");
    assert_eq!(result["by_layer"][0]["count"], 3);
    assert_eq!(result["bounds"]["min"], json!({"x": 0.0, "y": 0.0}));
    assert_eq!(result["bounds"]["max"], json!({"x": 11.0, "y": 6.0}));
    assert_eq!(result["revision"], state.session().revision().to_string());
}

#[test]
fn text_is_what_a_reader_sees_on_one_line_and_bounded() {
    let mut state = AppState::new();
    state.add_entity(text("{\\fArial;PIPE}\\PSupport\u{7}", Point::new(1.0, 2.0)));
    state.add_entity(text(&"x".repeat(500), Point::new(3.0, 4.0)));
    let result = summary(&state, json!({})).unwrap();
    assert_eq!(result["texts_total"], 2);
    let first = &result["texts"][0];
    assert_eq!(first["text"], "PIPE / Support ", "{result}");
    assert_eq!(first["at"], json!({"x": 1.0, "y": 2.0}));
    assert_eq!(
        result["texts"][1]["text"].as_str().unwrap().chars().count(),
        120
    );
    for t in result["texts"].as_array().unwrap() {
        assert!(!t["text"].as_str().unwrap().chars().any(char::is_control));
    }
}

#[test]
fn lists_are_cut_to_the_limit_and_the_totals_stay_complete() {
    let mut state = AppState::new();
    for i in 0..5 {
        state.add_entity(text(&format!("note {i}"), Point::new(f64::from(i), 0.0)));
    }
    let result = summary(&state, json!({"limit": 2})).unwrap();
    assert_eq!(result["texts"].as_array().unwrap().len(), 2);
    assert_eq!(result["texts_total"], 5);
}

#[test]
fn block_references_are_counted_by_name_with_where_the_first_is() {
    let mut state = AppState::new();
    state.add_entity(Entity::Rectangle {
        min: Point::new(10.0, 10.0),
        max: Point::new(12.0, 11.0),
    });
    state.select_all();
    state.start_command("BLOCK", crate::block_cmd::define_block);
    state.settle_command();
    for answer in ["Box", "10,10"] {
        state.cmd_submit(answer);
        state.settle_command();
    }
    let block = state.session().document().blocks()[0].clone();
    state.add_entity(Entity::Insert(Box::new(Insert::new(
        block,
        Point::new(50.0, 5.0),
    ))));
    let result = summary(&state, json!({})).unwrap();
    assert_eq!(result["blocks_total"], 1);
    assert_eq!(result["blocks"][0]["name"], "Box");
    assert_eq!(
        result["blocks"][0]["count"], 2,
        "the one the command made, and this"
    );
    assert!(result["by_kind"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["kind"] == "block_reference"));
}

#[test]
fn the_density_grid_shows_where_the_objects_are_and_covers_a_requested_window() {
    let mut state = AppState::new();
    // Two objects bottom left, one top right of a 0..80 square.
    state.add_entity(line(1.0, 1.0));
    state.add_entity(line(2.0, 2.0));
    state.add_entity(line(78.0, 79.0));
    let result = summary(&state, json!({})).unwrap();
    let grid = &result["density"];
    assert_eq!(
        (grid["columns"].as_u64(), grid["rows"].as_u64()),
        (Some(8), Some(8))
    );
    assert_eq!(grid["counts"][0][0], 2, "row 0 is the bottom: {grid}");
    assert_eq!(grid["counts"][7][7], 1);
    let total: u64 = grid["counts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .map(|c| c.as_u64().unwrap())
        .sum();
    assert_eq!(total, 3, "each object counts once");

    let window = json!({"min_x": 0, "min_y": 0, "max_x": 8, "max_y": 8});
    let near = summary(&state, json!({"bounds": window})).unwrap();
    assert_eq!(near["scanned"], 2);
    assert_eq!(
        near["density"]["bounds"]["max"],
        json!({"x": 8.0, "y": 8.0}),
        "the grid covers the window"
    );
    assert_eq!(near["density"]["counts"][1][1], 1);
}

#[test]
fn an_empty_drawing_has_no_box_and_no_grid_and_bad_arguments_are_refused() {
    let state = AppState::new();
    let empty = summary(&state, json!({})).unwrap();
    assert_eq!(empty["scanned"], 0);
    assert!(
        empty.get("bounds").is_none() && empty.get("density").is_none(),
        "{empty}"
    );
    let bad = json!({"bounds": {"min_x": 5, "min_y": 0, "max_x": 1, "max_y": 1}});
    assert_eq!(
        summary(&state, bad).unwrap_err().code,
        ErrorCode::InvalidArguments
    );
    let layout = json!({"space": {"kind": "paper", "layout_id": "999"}});
    assert_eq!(
        summary(&state, layout).unwrap_err().code,
        ErrorCode::InvalidReference
    );
    assert_eq!(
        summary(&state, json!({"depth": 2})).unwrap_err().code,
        ErrorCode::InvalidArguments
    );
}
