//! The numbers inside character controls (`\H2.5;`, `\C1;`).

use super::parse::{Format, Scalar};

/// A finite number from `text`, with a trailing `x` marking a multiple.
fn scalar(text: &str) -> Option<Scalar> {
    let (digits, relative) = match text.strip_suffix('x') {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    let value: f64 = digits.parse().ok()?;
    (value.is_finite() && value.abs() <= 1e9).then_some(Scalar { value, relative })
}

pub(super) fn format_of(letter: char, payload: &str) -> Option<Format> {
    let positive = |s: Scalar| (s.value > 0.0).then_some(s);
    match letter {
        'H' => scalar(payload).and_then(positive).map(Format::Height),
        'W' => scalar(payload).and_then(positive).map(Format::Width),
        'T' => scalar(payload).and_then(positive).map(Format::Tracking),
        'Q' => scalar(payload)
            .filter(|s| !s.relative)
            .map(|s| Format::Oblique(s.value)),
        'C' => payload.parse().ok().filter(|n| *n <= 256).map(Format::Aci),
        'c' => payload
            .parse()
            .ok()
            .filter(|n| *n <= 0xFF_FFFF)
            .map(Format::TrueColor),
        'A' => payload.parse().ok().filter(|n| *n <= 2).map(Format::Align),
        _ => None,
    }
}
