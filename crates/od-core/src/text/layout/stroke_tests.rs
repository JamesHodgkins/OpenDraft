//! Tests of laying text out in OD Roman.

use super::tests::{lay, text, H};
use super::*;
use crate::text::{glyph_outline, od_roman, FontFace};
use crate::text_style::TextStyle;

fn roman(bold: bool, italic: bool) -> TextStyle {
    TextStyle {
        font: FontDescriptor::OdRoman { bold, italic },
        ..TextStyle::standard()
    }
}

fn lay_roman(contents: &str, style: &TextStyle) -> TextLayout {
    layout_text(&text(contents), style)
}

fn top_of(g: &PositionedGlyph) -> f64 {
    glyph_outline(g.face, g.id, g.weight, 0.5)
        .iter()
        .flatten()
        .map(|&(fx, fy)| g.place(fx, fy).1)
        .fold(f64::MIN, f64::max)
}

#[test]
fn a_style_in_od_roman_lays_out_stroke_glyphs() {
    let l = lay_roman("Hi", &roman(false, false));
    assert_eq!(l.glyphs.len(), 2);
    assert!(l
        .glyphs
        .iter()
        .all(|g| g.face == FontFace::OdRoman && g.weight == 100));
    assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
    assert_eq!(l.glyphs[0].id, od_roman().unwrap().slot_of('H').unwrap());
}

#[test]
fn the_other_font_has_no_weight() {
    assert!(lay(&text("Hi")).glyphs.iter().all(|g| g.weight == 0));
}

#[test]
fn a_capital_is_the_text_height_tall_plus_half_the_pen() {
    let l = lay_roman("H", &roman(false, false));
    let g = &l.glyphs[0];
    // The baseline is `-H` below the block's top (a line is as tall as its text).
    let rise = top_of(g) - g.y;
    let half_pen = 0.10 * H / 2.0;
    assert!((rise - (H + half_pen)).abs() < 0.05 * H, "{rise}");
}

#[test]
fn weight_never_reflows_text() {
    let widths: Vec<f64> = [0.02, 0.10, 0.30]
        .iter()
        .map(|&w| {
            let style = TextStyle {
                stroke_weight: w,
                ..roman(false, false)
            };
            lay_roman("Hamburgefonts 0123", &style).actual_width
        })
        .collect();
    assert!(widths[0] > 0.0);
    assert_eq!(widths[0], widths[1]);
    assert_eq!(widths[1], widths[2]);
}

#[test]
fn the_style_weight_is_the_glyph_weight_and_bold_thickens_it() {
    let style = TextStyle {
        stroke_weight: 0.2,
        ..roman(false, false)
    };
    assert_eq!(lay_roman("a", &style).glyphs[0].weight, 200);
    let bold = TextStyle {
        stroke_weight: 0.2,
        ..roman(true, false)
    };
    assert_eq!(lay_roman("a", &bold).glyphs[0].weight, 320);
    let clamped = TextStyle {
        stroke_weight: 0.3,
        ..roman(true, false)
    };
    assert_eq!(lay_roman("a", &clamped).glyphs[0].weight, 400);
}

#[test]
fn italic_adds_a_fixed_slant_to_any_oblique() {
    let upright = lay_roman("a", &roman(false, false)).glyphs[0].shear;
    assert_eq!(upright, 0.0);
    let italic = lay_roman("a", &roman(false, true)).glyphs[0].shear;
    assert!((italic - 15.0_f64.to_radians().tan()).abs() < 1e-9);
    let both = TextStyle {
        oblique_radians: 10.0_f64.to_radians(),
        ..roman(false, true)
    };
    let sum = lay_roman("a", &both).glyphs[0].shear;
    assert!((sum - 25.0_f64.to_radians().tan()).abs() < 1e-9);
}

#[test]
fn a_run_oblique_shears_od_roman_like_any_font() {
    let l = lay_roman("a\\Q20;b", &roman(false, false));
    assert_eq!(l.glyphs[0].shear, 0.0);
    assert!((l.glyphs[1].shear - 20.0_f64.to_radians().tan()).abs() < 1e-9);
}

#[test]
fn a_shear_does_not_move_the_advances() {
    let plain = lay_roman("Hamburg", &roman(false, false));
    let slanted = lay_roman("Hamburg", &roman(false, true));
    assert_eq!(plain.actual_width, slanted.actual_width);
}

#[test]
fn a_space_takes_its_advance_from_the_font() {
    let font = od_roman().unwrap();
    let style = roman(false, false);
    // A trailing space hangs past the line, so measure one between letters.
    let gap = lay_roman("a a", &style).actual_width - lay_roman("aa", &style).actual_width;
    let want = font.metrics.space * H / font.metrics.cap_height;
    assert!((gap - want).abs() < 1e-9, "{gap} vs {want}");
    assert!(lay_roman(" ", &style).glyphs.is_empty());
}

#[test]
fn a_character_the_font_lacks_is_drawn_in_the_bundled_font() {
    let l = lay_roman("a$b", &roman(false, false));
    let faces: Vec<_> = l.glyphs.iter().map(|g| g.face).collect();
    assert_eq!(
        faces,
        [FontFace::OdRoman, FontFace::Regular, FontFace::OdRoman]
    );
    assert_eq!(l.glyphs[1].weight, 0);
    assert!(l.diagnostics.is_empty(), "DejaVu has a dollar sign");
    // Nobody has this one: say so.
    let l = lay_roman("a\u{E000}", &roman(false, false));
    assert!(l
        .diagnostics
        .iter()
        .any(|d| d.issue == LayoutIssue::MissingGlyph));
}

#[test]
fn a_font_control_switches_between_the_fonts_in_one_text() {
    let l = lay(&text("a\\fOD Roman;b\\fDejaVu Sans;c"));
    assert_eq!(
        l.glyphs.iter().map(|g| g.face).collect::<Vec<_>>(),
        [FontFace::Regular, FontFace::OdRoman, FontFace::Regular]
    );
    assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
    let l = lay_roman("a\\fArial;b", &roman(false, false));
    assert!(l
        .diagnostics
        .iter()
        .any(|d| d.issue == LayoutIssue::FontSubstituted));
    assert!(l.glyphs.iter().all(|g| g.face == FontFace::OdRoman));
}

#[test]
fn a_font_control_bold_thickens_od_roman() {
    let l = lay_roman("a\\fOD Roman|b1;b", &roman(false, false));
    assert_eq!(l.glyphs[0].weight, 100);
    assert_eq!(l.glyphs[1].weight, 160);
}

#[test]
fn a_stack_in_od_roman_keeps_its_font_bar_and_one_unit() {
    let l = lay_roman("\\S1/2;", &roman(false, false));
    assert_eq!(l.glyphs.len(), 2);
    assert!(l.glyphs.iter().all(|g| g.face == FontFace::OdRoman));
    assert_eq!(l.decorations.len(), 1);
}

#[test]
fn the_text_extent_and_carets_still_follow_the_layout() {
    let l = lay_roman("Hello", &roman(false, true));
    assert_eq!(l.carets.len(), lay(&text("Hello")).carets.len());
    assert!(l.actual_width > 0.0 && l.actual_height >= H);
    assert!(l.carets.windows(2).all(|w| w[0].x < w[1].x));
}

#[test]
fn text_wraps_at_spaces_in_od_roman() {
    let m = super::tests::wrapped("one two three", 5.0);
    let l = layout_text(&m, &roman(false, false));
    assert!(l.lines.len() >= 2, "{}", l.lines.len());
}

#[test]
fn remembered_layouts_tell_two_weights_apart() {
    let m = text("a");
    let thin = TextStyle {
        stroke_weight: 0.05,
        ..roman(false, false)
    };
    let thick = TextStyle {
        stroke_weight: 0.25,
        ..roman(false, false)
    };
    assert_eq!(m.layout(&thin).glyphs[0].weight, 50);
    assert_eq!(m.layout(&thick).glyphs[0].weight, 250);
}
