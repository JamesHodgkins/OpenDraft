//! Tests of fonts faces and stacked text in the layout.

use super::tests::{glyph_of, lay, line_texts, text, wrapped, H};
use super::*;
use crate::text::{glyph_contours, FontFace};
use crate::text_style::TextStyle;

fn faces(layout: &TextLayout) -> Vec<FontFace> {
    layout.glyphs.iter().map(|g| g.face).collect()
}

#[test]
fn font_controls_pick_the_bold_and_italic_faces() {
    let l = lay(&text(
        "a\\fDejaVu Sans|b1;b\\fDejaVu Sans|i1;c\\fDejaVu Sans|b1|i1;d\\fDejaVu Sans;e",
    ));
    assert_eq!(
        faces(&l),
        [
            FontFace::Regular,
            FontFace::Bold,
            FontFace::Italic,
            FontFace::BoldItalic,
            FontFace::Regular
        ]
    );
    assert!(
        l.diagnostics.is_empty(),
        "the bundled family needs no apology"
    );
    // A group puts the face back.
    let l = lay(&text("{\\fDejaVu Sans|b1;a}b"));
    assert_eq!(faces(&l), [FontFace::Bold, FontFace::Regular]);
}

#[test]
fn another_family_is_drawn_in_the_bundled_one_and_says_so() {
    let l = lay(&text("\\fArial|b1;a"));
    assert_eq!(faces(&l), [FontFace::Bold]);
    assert!(l
        .diagnostics
        .iter()
        .any(|d| d.issue == LayoutIssue::FontSubstituted));
}

#[test]
fn a_bold_run_is_wider_and_every_face_keeps_the_cap_height() {
    let plain = lay(&text("iiii"));
    let bold = lay(&text("\\fDejaVu Sans|b1;iiii"));
    assert!(bold.actual_width > plain.actual_width * 1.05);
    for control in [
        "",
        "\\fDejaVu Sans|b1;",
        "\\fDejaVu Sans|i1;",
        "\\fDejaVu Sans|b1|i1;",
    ] {
        let l = lay(&text(&format!("{control}H")));
        let g = &l.glyphs[0];
        let top = glyph_contours(g.face, g.id, 1.0)
            .iter()
            .flatten()
            .map(|p| g.place(p.0, p.1).1)
            .fold(f64::MIN, f64::max);
        assert!(
            top.abs() < 1e-6,
            "{control}: the capital reaches the top, {top}"
        );
    }
    let regular = glyph_contours(FontFace::Regular, glyph_of('O'), 1.0);
    let heavy = glyph_contours(FontFace::Bold, glyph_of('O'), 1.0);
    assert_ne!(regular, heavy);
}

#[test]
fn a_style_can_ask_for_bold_and_italic_and_a_font_control_overrides_it() {
    let style = TextStyle {
        font: FontDescriptor::TrueType {
            family: "DejaVu Sans".into(),
            bold: true,
            italic: true,
        },
        ..TextStyle::standard()
    };
    let l = layout_text(&text("a\\fDejaVu Sans;b"), &style);
    assert_eq!(faces(&l), [FontFace::BoldItalic, FontFace::Regular]);
    assert!(l.diagnostics.is_empty());
}

fn bars(layout: &TextLayout) -> Vec<&Decoration> {
    layout
        .decorations
        .iter()
        .filter(|d| d.kind == DecorationKind::FractionBar)
        .collect()
}

#[test]
fn a_fraction_is_two_smaller_runs_centred_over_a_bar() {
    let l = lay(&text("\\S1/22;"));
    assert_eq!(l.glyphs.len(), 3);
    let (one, two) = (&l.glyphs[0], &l.glyphs[1]);
    assert!((one.scale_y / l.glyphs[1].scale_y - 1.0).abs() < 1e-9);
    // At 70% of the height, the numerator above the baseline, the denominator below.
    let plain = lay(&text("1"));
    assert!((one.scale_y / plain.glyphs[0].scale_y - 0.7).abs() < 1e-9);
    assert!(one.y > l.lines[0].baseline && two.y < l.lines[0].baseline);
    let bar = bars(&l);
    assert_eq!(bar.len(), 1);
    assert!(bar[0].y > l.lines[0].baseline && bar[0].y < one.y + 0.7 * H);
    assert_eq!(bar[0].x0, 0.0);
    assert!(
        (bar[0].x1 - l.actual_width).abs() < 1e-9,
        "the bar spans the stack"
    );
    // The shorter part is centred under the longer.
    let narrow = l.glyphs[0].x;
    let wide_start = l.glyphs[1].x;
    assert!(
        narrow > 0.0 && wide_start > 0.0 && narrow > wide_start - 1e-9 || narrow < wide_start + 1.0
    );
}

#[test]
fn a_stack_is_one_unit_in_the_line_and_the_text_around_it_follows() {
    let l = lay(&text("x\\S1/2;y"));
    assert_eq!(l.glyphs.len(), 4);
    let (x_end, y_start) = (l.glyphs[0].x + 1.0, l.glyphs[3].x);
    assert!(y_start > x_end, "y comes after the whole stack");
    assert_eq!(l.lines.len(), 1);
    // It never breaks, even where a plain "1/2" would not either, and a
    // narrow box puts it on its own line with its neighbours.
    let m = wrapped("one \\S10/20; two", 3.0);
    let lines = line_texts(&lay(&m), &m.contents);
    assert_eq!(lines.len(), 3);
    assert!(lines[1].starts_with("\\S"));
}

#[test]
fn the_other_dividers_have_no_bar_or_a_slant() {
    let tolerance = lay(&text("\\S+1^-2;"));
    assert!(bars(&tolerance).is_empty(), "^ stacks without a bar");
    assert!(tolerance.glyphs[0].y > tolerance.glyphs[tolerance.glyphs.len() - 1].y);
    let slanted = lay(&text("\\S1#2;"));
    assert!(bars(&slanted).is_empty());
    assert_eq!(slanted.glyphs.len(), 3, "numerator, slash, denominator");
    assert!(slanted.glyphs[0].x < slanted.glyphs[1].x && slanted.glyphs[1].x < slanted.glyphs[2].x);
    assert!(slanted.glyphs[0].y > slanted.glyphs[2].y);
    // With no divider there is nothing to stack: it reads as text.
    let none = lay(&text("\\Sabc;"));
    assert!(bars(&none).is_empty());
    assert_eq!(none.glyphs.len(), 3);
    assert!((none.glyphs[0].scale_y - lay(&text("a")).glyphs[0].scale_y).abs() < 1e-9);
}

#[test]
fn a_stack_takes_the_formatting_round_it() {
    let l = lay(&text("\\C1;\\H2x;\\S1/2;"));
    assert_eq!(l.glyphs[0].color, TextColor::Aci(1));
    let plain = lay(&text("\\S1/2;"));
    assert!((l.glyphs[0].scale_y / plain.glyphs[0].scale_y - 2.0).abs() < 1e-9);
    assert_eq!(bars(&l)[0].color, TextColor::Aci(1));
    let underlined = lay(&text("\\L\\S1/2;"));
    assert!(underlined
        .decorations
        .iter()
        .any(|d| d.kind == DecorationKind::Underline));
}

#[test]
fn a_stack_on_the_last_line_is_inside_the_frame() {
    let plain = lay(&text("1"));
    let stacked = lay(&text("\\S1/2;"));
    assert!(
        stacked.actual_height > plain.actual_height,
        "the denominator hangs lower"
    );
    let lowest = stacked.glyphs.iter().map(|g| g.y).fold(f64::MAX, f64::min);
    assert!(-lowest <= stacked.actual_height + 1e-9);
}

#[test]
fn a_stack_is_one_caret_step_and_one_delete() {
    let l = lay(&text("x\\S1/2;y"));
    let sources: Vec<usize> = l.carets.iter().map(|c| c.source).collect();
    assert_eq!(sources, [0, 1, 7, 8]);
    assert_eq!(
        (l.carets[1].end, l.carets[1].source),
        (7, 1),
        "a delete there takes the whole stack"
    );
}
