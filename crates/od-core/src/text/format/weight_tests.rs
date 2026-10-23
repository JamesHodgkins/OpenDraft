//! Tests of pen weights in character edits, and of reusing the group an
//! edit lands in.

use std::ops::Range;

use super::tests::{bold, fmt, underline, H};
use super::*;
use crate::mtext::MText;
use crate::text::layout_text;
use crate::text_style::{TextStyle, TextStyleId};
use crate::Point;

fn weight(w: u16) -> CharEdit {
    CharEdit {
        weight: Some(w),
        ..CharEdit::default()
    }
}

fn roman(contents: &str, range: Range<usize>, edit: &CharEdit) -> Formatted {
    apply_char_edit(contents, range, edit, "OD Roman").unwrap()
}

#[test]
fn a_weight_is_written_in_the_font_control_and_reads_back() {
    let done = roman("hello", 1..4, &weight(180));
    assert_eq!(done.contents, "h{\\fOD Roman|b0|i0|w0.18;ell}o");
    assert_eq!(
        fmt(&done.contents, done.range.clone()).weight,
        Mixed::Uniform(Some(180))
    );
    assert_eq!(fmt(&done.contents, 0..1).weight, Mixed::Uniform(None));
    // Bold on top keeps the weight; a heavier weight replaces it.
    let bolder = roman(&done.contents, done.range.clone(), &bold(true));
    assert_eq!(
        fmt(&bolder.contents, bolder.range.clone()).weight,
        Mixed::Uniform(Some(180))
    );
    let heavier = roman(&bolder.contents, bolder.range.clone(), &weight(300));
    let f = fmt(&heavier.contents, heavier.range.clone());
    assert_eq!(
        (f.weight, f.bold),
        (Mixed::Uniform(Some(300)), Mixed::Uniform(true))
    );
    // 0 hands it back to the style.
    let reset = roman(&heavier.contents, heavier.range.clone(), &weight(0));
    assert_eq!(
        fmt(&reset.contents, reset.range).weight,
        Mixed::Uniform(None)
    );
}

#[test]
fn a_weight_outside_the_range_is_refused() {
    for bad in [5, 401, 9999] {
        assert_eq!(
            apply_char_edit("x", 0..1, &weight(bad), "OD Roman"),
            Err(EditRefusal::BadValue)
        );
    }
    assert!(apply_char_edit("x", 0..1, &weight(10), "OD Roman").is_ok());
    assert!(apply_char_edit("x", 0..1, &weight(400), "OD Roman").is_ok());
}

#[test]
fn a_pending_weight_shows_before_it_is_typed_and_the_layout_draws_it() {
    let pending = fmt("ab", 0..0).with(&weight(250));
    assert_eq!(pending.weight, Mixed::Uniform(Some(250)));
    let done = roman("abc", 1..2, &weight(250));
    let text = MText::new(
        Point::new(0.0, 0.0),
        H,
        done.contents,
        TextStyleId::STANDARD,
    );
    let style = TextStyle {
        font: crate::FontDescriptor::OdRoman {
            bold: false,
            italic: false,
        },
        ..TextStyle::standard()
    };
    let weights: Vec<_> = layout_text(&text, &style)
        .glyphs
        .iter()
        .map(|g| g.weight)
        .collect();
    assert_eq!(weights, [100, 250, 100]);
}

#[test]
fn a_selection_that_is_exactly_a_group_reuses_it() {
    let first = roman("hello", 0..5, &weight(100));
    // Select All takes in the braces of the group.
    let whole = 0..first.contents.len();
    let second = roman(&first.contents, whole, &weight(200));
    assert_eq!(
        second.contents.matches('{').count(),
        1,
        "{}",
        second.contents
    );
    let f = fmt(&second.contents, second.range.clone());
    assert_eq!(f.weight, Mixed::Uniform(Some(200)));
    assert_eq!(&second.contents[second.range.clone()], "hello");
    // Groups inside groups are peeled the same way.
    let nested = format!("{{{}}}", first.contents);
    let third = roman(&nested, 0..nested.len(), &weight(300));
    assert_eq!(third.contents.matches('{').count(), 2, "{}", third.contents);
    assert_eq!(&third.contents[third.range.clone()], "hello");
}

#[test]
fn an_edit_replaces_the_controls_it_overrides_and_keeps_the_rest() {
    let mut now = roman("hello", 0..5, &weight(100));
    now = apply_char_edit(
        &now.contents,
        now.range.clone(),
        &underline(true),
        "OD Roman",
    )
    .unwrap();
    let before = now.contents.len();
    for w in (10..=400).chain((10..=400).rev()) {
        now = roman(&now.contents, now.range.clone(), &weight(w));
    }
    assert!(now.contents.len() <= before + 5, "{}", now.contents);
    assert_eq!(now.contents.matches(r"\f").count(), 1, "{}", now.contents);
    assert_eq!(now.contents.matches(r"\L").count(), 1, "{}", now.contents);
    let f = fmt(&now.contents, now.range.clone());
    assert_eq!(f.weight, Mixed::Uniform(Some(10)));
    assert_eq!(f.underline, Mixed::Uniform(true));
}

#[test]
fn an_empty_group_is_left_alone() {
    let done = roman("a{}b", 1..3, &weight(100));
    assert_eq!(done.contents, "a{}b");
}
