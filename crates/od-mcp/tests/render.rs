//! The real `opendraft-mcp` binary: a picture comes back once, outside the
//! structured part, in the form the assistant can use.

#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

mod support;

use serde_json::json;
use support::{scratch, FakeApp, Sidecar, TOKEN};

#[test]
fn a_picture_comes_back_once_as_an_svg_resource() {
    let dir = scratch("render");
    let _app = FakeApp::start(&dir, "hhhh", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call(
        "opendraft_view_render",
        json!({"document_id": "doc_fake_1"}),
    );
    assert_ne!(result["isError"], true, "{result}");
    assert!(
        result["structuredContent"].get("svg").is_none(),
        "the image is not repeated in the structured part: {result}"
    );
    assert_eq!(result["structuredContent"]["revision"], "7");
    let resource = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "resource")
        .unwrap_or_else(|| panic!("no resource in {result}"));
    assert_eq!(resource["resource"]["mimeType"], "image/svg+xml");
    assert_eq!(resource["resource"]["text"], "<svg/>");
    assert_eq!(
        resource["resource"]["uri"],
        "opendraft://render/revision-7.svg"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_png_comes_back_once_as_an_image_a_model_can_look_at() {
    let dir = scratch("png");
    let app = FakeApp::start(&dir, "pppp", TOKEN);
    let mut sidecar = Sidecar::start(&dir, &[]);
    sidecar.initialise();
    let result = sidecar.call(
        "opendraft_view_render",
        json!({"document_id": "doc_fake_1", "format": "png"}),
    );
    assert_ne!(result["isError"], true, "{result}");
    assert!(
        result["structuredContent"].get("png_base64").is_none(),
        "the picture is not repeated in the structured part: {result}"
    );
    assert_eq!(result["structuredContent"]["format"], "png");
    let image = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .unwrap_or_else(|| panic!("no image in {result}"));
    assert_eq!(image["mimeType"], "image/png");
    assert_eq!(image["data"], support::TINY_PNG);
    assert!(
        result["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["type"] != "resource"),
        "no SVG resource for a PNG: {result}"
    );
    // The format reaches the app as asked.
    let seen = app.seen.lock().unwrap();
    assert_eq!(seen.last().unwrap().arguments["format"], "png");
    drop(seen);
    let _ = std::fs::remove_dir_all(&dir);
}
