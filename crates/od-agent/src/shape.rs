//! What the model sees of a tool's result: the result as compact text, cut to
//! a visible limit, with a picture taken out and passed as a picture. The
//! app's answer is never trusted to be small.

use od_api::catalogue;
use od_api::ApiError;
use serde_json::Value;

use crate::budget::{BudgetKind, Meter};
use crate::types::{Content, Image};

/// Cut `text` to at most `max` bytes at a character boundary, and say so.
pub fn cut(text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let total = text.len();
    format!(
        "{}\n[cut: {end} of {total} bytes shown; ask for less at a time]",
        &text[..end]
    )
}

/// A finished call as the model is told of it. A picture in any result (a
/// render, or the look the app attaches to a change) is taken out and passed
/// as a picture, named by the result's `picture_of` if it has one. A full
/// conversation is an error, not a cut: the task ends with the reason rather
/// than carrying on blind.
pub fn done(mut result: Value, meter: &mut Meter) -> Result<Vec<Content>, BudgetKind> {
    let each = meter.budgets().result_bytes_each;
    let mut content = Vec::new();
    let mut images = Vec::new();
    let label = result
        .as_object_mut()
        .and_then(|map| map.remove("picture_of"))
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "the drawing as it looks on the sheet".to_owned());
    let pictures = catalogue::take_pictures(&mut result);
    if let Some(svg) = pictures.svg {
        content.push(Content::Text(cut(
            format!(
                "SVG of the view:
{svg}"
            ),
            each,
        )));
    }
    if let Some(base64) = pictures.png_base64 {
        if meter.take_picture() {
            images.push(Content::Image(Image {
                media_type: "image/png".to_owned(),
                base64,
                label,
            }));
        } else {
            content.push(Content::Text(
                "[picture not shown: this task has used all the pictures it may]".to_owned(),
            ));
        }
    }
    let text = serde_json::to_string(&result).unwrap_or_else(|_| "null".to_owned());
    content.insert(0, Content::Text(cut(text, each)));
    let kept: usize = content
        .iter()
        .map(|c| match c {
            Content::Text(t) => t.len(),
            Content::Image(_) => 0,
        })
        .sum();
    meter.charge_result_bytes(kept)?;
    content.extend(images);
    Ok(content)
}

/// An error the app returned, as text the model can act on.
pub fn failed(error: &ApiError) -> Vec<Content> {
    let text = serde_json::to_string(error).unwrap_or_else(|_| error.message.clone());
    vec![Content::Text(text)]
}

/// The harness refusing a call it will not pass on.
pub fn refused(message: &str) -> Vec<Content> {
    vec![Content::Text(
        serde_json::json!({"error": message}).to_string(),
    )]
}
