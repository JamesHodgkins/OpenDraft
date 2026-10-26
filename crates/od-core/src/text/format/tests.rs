use super::*;
use crate::mtext::MText;
use crate::text::{layout_text, DecorationKind};
use crate::text_style::{TextStyle, TextStyleId};
use crate::Point;

pub(super) const H: f64 = 2.0;
const FAMILY: &str = "DejaVu Sans";

pub(super) fn fmt(contents: &str, range: Range<usize>) -> SelectionFormat {
    format_in(contents, H, range)
}

pub(super) fn apply(contents: &str, range: Range<usize>, edit: CharEdit) -> Formatted {
    apply_char_edit(contents, range, &edit, FAMILY).unwrap()
}

pub(super) fn underline(on: bool) -> CharEdit {
    CharEdit {
        underline: Some(on),
        ..CharEdit::default()
    }
}

#[test]
fn a_caret_looks_like_the_text_before_it() {
    // a(0) \L(1..3) b(3)
    let src = "a\\Lb";
    assert_eq!(fmt(src, 1..1).underline, Mixed::Uniform(false));
    assert_eq!(
        fmt(src, 3..3).underline,
        Mixed::Uniform(true),
        "after the control"
    );
    assert_eq!(fmt("", 0..0).height, Mixed::Uniform(H));
    assert_eq!(fmt("\\H5;x", 5..5).height, Mixed::Uniform(5.0));
}

#[test]
fn a_selection_is_uniform_or_mixed() {
    let src = "a\\LB\\l c";
    assert_eq!(fmt(src, 0..4).underline, Mixed::Mixed);
    assert_eq!(fmt(src, 3..4).underline, Mixed::Uniform(true));
    assert_eq!(fmt(src, 6..src.len()).underline, Mixed::Uniform(false));
    // Only controls selected: what the caret there sees.
    assert_eq!(fmt(src, 1..3).underline, Mixed::Uniform(false));
    let sizes = "\\H4;ab\\H2;cd";
    assert_eq!(fmt(sizes, 0..sizes.len()).height, Mixed::Mixed);
    assert_eq!(fmt(sizes, 4..6).height, Mixed::Uniform(4.0));
    let relative = "\\H2x;ab";
    assert_eq!(fmt(relative, 5..7).height, Mixed::Uniform(2.0 * H));
}

#[test]
fn groups_end_their_formatting() {
    let src = "{\\La}b";
    assert_eq!(fmt(src, 3..4).underline, Mixed::Uniform(true));
    assert_eq!(
        fmt(src, 5..6).underline,
        Mixed::Uniform(false),
        "b is outside the group"
    );
    assert_eq!(
        fmt(src, 4..4).underline,
        Mixed::Uniform(true),
        "the caret just after a"
    );
    let colours = "\\C1;a\\c255;b\\C256;c";
    assert_eq!(fmt(colours, 4..5).color, Mixed::Uniform(TextColor::Aci(1)));
    assert_eq!(
        fmt(colours, 11..12).color,
        Mixed::Uniform(TextColor::Rgb(255))
    );
    assert_eq!(
        fmt(colours, 18..19).color,
        Mixed::Uniform(TextColor::Entity)
    );
    let shape = "\\Q15;a\\T2;b\\W0.5;c";
    assert_eq!(fmt(shape, 5..6).oblique, Mixed::Uniform(15.0));
    assert_eq!(fmt(shape, 10..11).tracking, Mixed::Uniform(2.0));
    assert_eq!(fmt(shape, 17..18).width, Mixed::Uniform(0.5));
}

#[test]
fn formatting_a_selection_wraps_it_in_a_group() {
    let done = apply("hello", 1..4, underline(true));
    assert_eq!(done.contents, "h{\\Lell}o");
    assert_eq!(&done.contents[done.range.clone()], "ell");
    assert_eq!(
        fmt(&done.contents, done.range.clone()).underline,
        Mixed::Uniform(true)
    );
    assert_eq!(fmt(&done.contents, 0..1).underline, Mixed::Uniform(false));
    assert_eq!(
        fmt(&done.contents, 8..9).underline,
        Mixed::Uniform(false),
        "after the group"
    );
}

#[test]
fn the_formatted_text_lays_out_with_the_formatting() {
    let done = apply("hello world", 6..11, underline(true));
    let text = MText::new(
        Point::new(0.0, 0.0),
        H,
        done.contents,
        TextStyleId::STANDARD,
    );
    let layout = layout_text(&text, &TextStyle::standard());
    let bars: Vec<_> = layout
        .decorations
        .iter()
        .filter(|d| d.kind == DecorationKind::Underline)
        .collect();
    assert_eq!(bars.len(), 1);
    assert!(
        bars[0].x0 > 0.0,
        "it starts at `world`, not at the start of the line"
    );
}

#[test]
fn changing_the_same_text_again_reuses_its_group() {
    let first = apply("hello", 1..4, underline(true));
    let second = apply(&first.contents, first.range.clone(), underline(false));
    assert_eq!(
        second.contents, "h{\\lell}o",
        "no second group, and the control it overrides is replaced"
    );
    assert_eq!(
        fmt(&second.contents, second.range.clone()).underline,
        Mixed::Uniform(false)
    );
    // Toggling many times never runs out of nesting.
    let mut now = first;
    for i in 0..30 {
        now = apply(&now.contents, now.range.clone(), underline(i % 2 == 0));
    }
    assert_eq!(now.contents.matches('{').count(), 1);
}

#[test]
fn every_setting_has_its_control() {
    let all = CharEdit {
        height: Some(3.5),
        underline: Some(true),
        overline: Some(true),
        strike: Some(false),
        bold: None,
        italic: None,
        color: Some(TextColor::Aci(3)),
        oblique: Some(12.0),
        tracking: Some(1.5),
        width: Some(0.8),
        weight: None,
    };
    assert_eq!(
        apply("x", 0..1, all).contents,
        "{\\L\\O\\k\\H3.5;\\C3;\\Q12;\\T1.5;\\W0.8;x}"
    );
    let colour = |c| {
        apply(
            "x",
            0..1,
            CharEdit {
                color: Some(c),
                ..CharEdit::default()
            },
        )
        .contents
    };
    assert_eq!(colour(TextColor::Entity), "{\\C256;x}");
    assert_eq!(colour(TextColor::Rgb(65280)), "{\\c65280;x}");
    // What is written reads back as what was asked for.
    let done = apply("abc", 0..3, all);
    let f = fmt(&done.contents, done.range);
    assert_eq!(f.height, Mixed::Uniform(3.5));
    assert_eq!(f.color, Mixed::Uniform(TextColor::Aci(3)));
    assert_eq!(f.strike, Mixed::Uniform(false));
    assert_eq!(f.overline, Mixed::Uniform(true));
    assert_eq!(f.width, Mixed::Uniform(0.8));
}

#[test]
fn edits_that_would_break_the_text_are_refused() {
    let u = underline(true);
    let refused = |src: &str, range: Range<usize>, edit: CharEdit| {
        apply_char_edit(src, range, &edit, FAMILY).unwrap_err()
    };
    assert_eq!(refused("a\\H2.5;b", 0..4, u), EditRefusal::BadRange);
    assert_eq!(refused("a\\pxi;b", 0..7, u), EditRefusal::Protected);
    assert_eq!(refused("ab", 0..9, u), EditRefusal::BadRange);
    for bad in [
        CharEdit {
            height: Some(0.0),
            ..CharEdit::default()
        },
        CharEdit {
            height: Some(f64::NAN),
            ..CharEdit::default()
        },
        CharEdit {
            tracking: Some(-1.0),
            ..CharEdit::default()
        },
        CharEdit {
            width: Some(f64::INFINITY),
            ..CharEdit::default()
        },
        CharEdit {
            oblique: Some(90.0),
            ..CharEdit::default()
        },
    ] {
        assert_eq!(refused("ab", 0..2, bad), EditRefusal::BadValue, "{bad:?}");
    }
    let deep = "{{{{{{{{xy}}}}}}}}";
    assert_eq!(refused(deep, 8..9, u), EditRefusal::TooDeep);
    assert!(
        apply_char_edit(deep, 8..10, &u, FAMILY).is_ok(),
        "the whole inside takes controls, not a ninth group"
    );
    let big = "a".repeat(1 << 20);
    assert_eq!(refused(&big, 0..5, u), EditRefusal::TooLarge);
}

#[test]
fn nothing_to_do_changes_nothing() {
    let same = apply("abc", 1..2, CharEdit::default());
    assert_eq!((same.contents.as_str(), same.range), ("abc", 1..2));
    let none = apply("abc", 2..2, underline(true));
    assert_eq!(
        none.contents, "abc",
        "an empty range is the caller's pending style"
    );
}

#[test]
fn edits_merge_and_overlay() {
    let mut pending = underline(true);
    pending.merge(&CharEdit {
        underline: Some(false),
        height: Some(4.0),
        ..CharEdit::default()
    });
    assert_eq!(pending.underline, Some(false));
    assert_eq!(pending.height, Some(4.0));
    assert!(!pending.is_empty() && CharEdit::default().is_empty());
    let shown = fmt("abc", 0..3).with(&pending);
    assert_eq!(shown.underline, Mixed::Uniform(false));
    assert_eq!(shown.height, Mixed::Uniform(4.0));
    assert_eq!(shown.strike, Mixed::Uniform(false));
    assert_eq!(Mixed::Uniform(3).uniform(), Some(3));
    assert_eq!(Mixed::<i32>::Mixed.uniform(), None);
}

#[test]
fn hostile_selections_never_panic_and_stay_readable() {
    const PIECES: &[&str] = &[
        "\\P", "\\L", "\\H3;", "{", "}", " ", "ab", "é", "\\S1/2;", "\\pxi;", "%<x>%", "\\\\",
        "\\C1;",
    ];
    let mut state = 0xfeed_beef_1234_5678_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..500 {
        let count = (next() % 14) as usize;
        let src: String = (0..count)
            .map(|_| PIECES[(next() % PIECES.len() as u64) as usize])
            .collect();
        let (a, b) = (
            (next() as usize) % (src.len() + 1),
            (next() as usize) % (src.len() + 1),
        );
        let range = a.min(b)..a.max(b);
        let _ = format_in(&src, H, range.clone());
        let nonempty = !range.is_empty();
        if let Ok(done) = apply_char_edit(&src, range, &underline(true), FAMILY) {
            if !nonempty {
                continue;
            }
            assert!(crate::text::parse(&done.contents).is_ok());
            assert!(done.contents.is_char_boundary(done.range.start));
            assert!(done.contents.is_char_boundary(done.range.end));
        }
    }
}

pub(super) fn bold(on: bool) -> CharEdit {
    CharEdit {
        bold: Some(on),
        ..CharEdit::default()
    }
}

fn italic(on: bool) -> CharEdit {
    CharEdit {
        italic: Some(on),
        ..CharEdit::default()
    }
}

#[test]
fn bold_and_italic_are_written_as_a_font_control() {
    let done = apply("hello", 1..4, bold(true));
    assert_eq!(done.contents, "h{\\fDejaVu Sans|b1|i0;ell}o");
    let f = fmt(&done.contents, done.range.clone());
    assert_eq!(
        (f.bold, f.italic),
        (Mixed::Uniform(true), Mixed::Uniform(false))
    );
    assert_eq!(fmt(&done.contents, 0..1).bold, Mixed::Uniform(false));
    // Italic on the same text keeps the bold, in the same group.
    let both = apply(&done.contents, done.range.clone(), italic(true));
    assert_eq!(both.contents, "h{\\fDejaVu Sans|b1|i1;ell}o");
    let f = fmt(&both.contents, both.range.clone());
    assert_eq!(
        (f.bold, f.italic),
        (Mixed::Uniform(true), Mixed::Uniform(true))
    );
    let off = apply(&both.contents, both.range, bold(false));
    let f = fmt(&off.contents, off.range);
    assert_eq!(
        (f.bold, f.italic),
        (Mixed::Uniform(false), Mixed::Uniform(true))
    );
}

#[test]
fn bold_keeps_the_font_the_text_already_has() {
    let src = "\\fArial|b0|i0|c204|p34;hello";
    let start = src.find("ell").unwrap();
    let done = apply(src, start..start + 3, bold(true));
    assert!(
        done.contents.contains("{\\fArial|b1|i0|c204|p34;ell}"),
        "{}",
        done.contents
    );
    // Without a font control it is written for the family it is given.
    let plain = apply_char_edit("ab", 0..2, &bold(true), "Calibri").unwrap();
    assert_eq!(plain.contents, "{\\fCalibri|b1|i0;ab}");
}

#[test]
fn a_group_gives_the_face_back_and_the_layout_uses_it() {
    let done = apply("abc", 1..2, bold(true));
    let text = MText::new(
        Point::new(0.0, 0.0),
        H,
        done.contents,
        TextStyleId::STANDARD,
    );
    let layout = layout_text(&text, &TextStyle::standard());
    let faces: Vec<_> = layout.glyphs.iter().map(|g| g.face).collect();
    use crate::text::FontFace::{Bold, Regular};
    assert_eq!(faces, [Regular, Bold, Regular]);
}

#[test]
fn a_selection_across_faces_is_mixed() {
    let src = "a\\fDejaVu Sans|b1;B\\fDejaVu Sans;c";
    assert_eq!(fmt(src, 0..src.len()).bold, Mixed::Mixed);
    let at = src.find('B').unwrap();
    assert_eq!(fmt(src, at..at + 1).bold, Mixed::Uniform(true));
    assert_eq!(fmt(src, src.len()..src.len()).bold, Mixed::Uniform(false));
}

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
