//! Character edits on selections that cut formatting groups.

use super::tests::{apply, fmt};
use super::*;

const FAMILY: &str = "DejaVu Sans";

fn width(v: f64) -> CharEdit {
    CharEdit {
        width: Some(v),
        ..CharEdit::default()
    }
}

#[test]
fn a_selection_that_ends_inside_a_group_formats_each_part_in_its_own_group() {
    // a{\\T2;bc}d, from the c to the d.
    let done = apply("a{\\T2;bc}d", 7..10, width(2.0));
    assert_eq!(done.contents, "a{\\T2;b{\\W2;c}}{\\W2;d}");
    assert_eq!(done.range, 12..22);
    let c = fmt(&done.contents, 12..13);
    assert_eq!(c.width, Mixed::Uniform(2.0));
    assert_eq!(c.tracking, Mixed::Uniform(2.0), "still in the spaced group");
    let d = fmt(&done.contents, 20..21);
    assert_eq!(d.width, Mixed::Uniform(2.0));
    assert_eq!(d.tracking, Mixed::Uniform(1.0));
    let b = fmt(&done.contents, 6..7);
    assert_eq!(b.width, Mixed::Uniform(1.0), "the b was not selected");
}

#[test]
fn a_selection_that_starts_before_a_group_and_ends_inside_it_works_too() {
    // a{\\T2;bc}d, from the a to the b.
    let done = apply("a{\\T2;bc}d", 0..8, width(2.0));
    assert_eq!(done.contents, "{\\W2;a}{\\T2;\\W2;bc}d");
}

#[test]
fn only_braces_selected_changes_nothing() {
    let src = "a{\\T2;bc}d";
    let done = apply(src, 8..9, width(2.0));
    assert_eq!(done.contents, src);
}

#[test]
fn changing_a_value_again_on_the_same_selection_does_not_nest_deeper() {
    let first = apply("a{\\T2;bc}d", 7..10, width(2.0));
    let second = apply(&first.contents, first.range.clone(), width(3.0));
    assert_eq!(second.contents, "a{\\T2;b{\\W3;c}}{\\W3;d}");
    let third = apply(&second.contents, second.range.clone(), width(4.0));
    assert_eq!(third.contents, "a{\\T2;b{\\W4;c}}{\\W4;d}");
}

#[test]
fn spacing_and_width_on_a_selection_across_many_groups_always_lands() {
    let src = "{\\T1.2;ab}cd{\\W2;ef}gh{\\L ij}";
    for start in 0..src.len() {
        for end in start + 1..=src.len() {
            if !src.is_char_boundary(start) || !src.is_char_boundary(end) {
                continue;
            }
            let tracking = CharEdit {
                tracking: Some(1.5),
                ..CharEdit::default()
            };
            match apply_char_edit(src, start..end, &tracking, FAMILY) {
                Ok(done) => {
                    // Whatever it made still reads as text with balanced groups.
                    let opens = done.contents.matches('{').count();
                    let closes = done.contents.matches('}').count();
                    assert_eq!(opens, closes, "{start}..{end}: {}", done.contents);
                }
                // Only cutting a control in two is still refused.
                Err(why) => assert_eq!(why, EditRefusal::BadRange, "{start}..{end}"),
            }
        }
    }
}

#[test]
fn stacking_text_with_half_a_group_in_it_says_it_cannot_be_stacked() {
    use crate::text::toggle_stack;
    // a{1/2}b, from the 1 to past the closing brace.
    assert_eq!(
        toggle_stack("a{1/2}b", 2..6).unwrap_err(),
        EditRefusal::CannotStack
    );
    assert!(toggle_stack("a1/2b", 1..4).is_ok());
}
