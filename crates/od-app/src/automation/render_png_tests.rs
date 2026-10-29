//! `view.render` as a PNG: a picture that can be looked at.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const ME: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn render(state: &mut AppState, mut args: Value) -> Result<Value, od_api::ApiError> {
    args["document_id"] = json!(document_id(INSTANCE, state.docs[state.active].uid));
    dispatch_test(state, INSTANCE, &ME, "view.render", args)
}

/// A line and a circle in a 120 by 100 window.
fn drawing() -> AppState {
    let mut state = AppState::new();
    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(100.0, 0.0),
    });
    state.add_entity(Entity::Circle {
        center: Point::new(50.0, 50.0),
        radius: 30.0,
    });
    state
}

const WINDOW: &str = r#"{"min_x": -10.0, "min_y": -10.0, "max_x": 110.0, "max_y": 90.0}"#;

/// Decode the picture in a result into (width, height, RGBA bytes).
fn pixels(result: &Value) -> (u32, u32, Vec<u8>) {
    let bytes = STANDARD
        .decode(result["png_base64"].as_str().unwrap())
        .unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut data = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut data).unwrap();
    (info.width, info.height, data)
}

#[test]
fn a_png_is_a_real_picture_of_the_same_window_with_its_size_stated() {
    let mut state = drawing();
    let window: Value = serde_json::from_str(WINDOW).unwrap();
    let result = render(
        &mut state,
        json!({"format": "png", "bounds": window, "width": 400.0}),
    )
    .unwrap();
    assert_eq!(result["format"], "png");
    assert!(result.get("svg").is_none(), "one picture, not both");
    assert_eq!(result["revision"], "2");
    assert_eq!(result["region"]["min"]["x"], -10.0);
    let (w, h, data) = pixels(&result);
    assert_eq!((f64::from(w), f64::from(h)), (400.0, 333.0));
    assert_eq!(
        (result["width"].as_f64(), result["height"].as_f64()),
        (Some(400.0), Some(333.0))
    );
    let dark = data
        .chunks(4)
        .filter(|p| p[0] < 128 && p[1] < 128 && p[2] < 128)
        .count();
    assert!(dark > 400, "the line and the circle are drawn: {dark}");
    assert_eq!(&data[..3], [255, 255, 255], "paper is white");
    assert!(result["fidelity"].as_str().unwrap().contains("rasterised"));
}

#[test]
fn the_picture_shows_the_drawing_the_right_way_up() {
    let mut state = drawing();
    let window: Value = serde_json::from_str(WINDOW).unwrap();
    let result = render(
        &mut state,
        json!({"format": "png", "bounds": window, "width": 400.0}),
    )
    .unwrap();
    let (w, h, data) = pixels(&result);
    let dark_at = |x: u32, y: u32| data[((y * w + x) * 4) as usize] < 128;
    // The line y = 0 is a tenth of the way up the window: near the bottom.
    let rows_with_ink: Vec<u32> = (0..h).filter(|y| dark_at(w / 20 * 3, *y)).collect();
    assert!(
        rows_with_ink.iter().any(|y| *y > h * 2 / 3),
        "the line is in the lower part: {rows_with_ink:?}"
    );
    // The circle's top (y = 80) is above its centre (y = 50): fewer rows down.
    let centre_column: Vec<u32> = (0..h).filter(|y| dark_at(w / 2 + w / 40, *y)).collect();
    assert!(
        centre_column.iter().any(|y| *y < h / 4),
        "the circle's top is in the upper part: {centre_column:?}"
    );
}

#[test]
fn svg_is_still_the_default_and_names_its_format() {
    let mut state = drawing();
    let result = render(&mut state, json!({})).unwrap();
    assert_eq!(result["format"], "svg");
    assert!(result["svg"].as_str().unwrap().starts_with("<svg"));
    assert!(result.get("png_base64").is_none());
}

#[test]
fn a_picture_too_big_in_pixels_is_refused_before_it_is_drawn() {
    let mut state = drawing();
    // 4096 by 4096 is more than the pixel limit.
    let square = json!({"min_x": 0, "min_y": 0, "max_x": 100, "max_y": 100});
    let error = render(
        &mut state,
        json!({"format": "png", "bounds": square, "width": 4096.0}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert!(error.message.contains("too large"), "{}", error.message);
    // A very tall window is too many rows however narrow.
    let tall = json!({"min_x": 0, "min_y": 0, "max_x": 1, "max_y": 100000});
    let error = render(
        &mut state,
        json!({"format": "png", "bounds": tall, "width": 800.0}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_eq!(
        render(&mut state, json!({"format": "bmp"}))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArguments
    );
}
