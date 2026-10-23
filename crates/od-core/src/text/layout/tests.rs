use std::rc::Rc;

use super::*;
use crate::text::{glyph_contours, FontFace, TextResource};
use crate::text_style::TextStyleId;
use crate::Point;

pub(super) const H: f64 = 2.0;
const PITCH: f64 = 5.0 / 3.0 * H;

pub(super) fn text(contents: &str) -> MText {
    MText::new(Point::new(0.0, 0.0), H, contents, TextStyleId::STANDARD)
}

pub(super) fn wrapped(contents: &str, width: f64) -> MText {
    MText {
        reference_width: width,
        ..text(contents)
    }
}

pub(super) fn lay(m: &MText) -> TextLayout {
    layout_text(m, &TextStyle::standard())
}

pub(super) fn glyph_of(c: char) -> u16 {
    font::face().unwrap().glyph_index(c).unwrap().0
}

/// The contents each line covers, with its break and spaces.
pub(super) fn line_texts<'a>(layout: &TextLayout, contents: &'a str) -> Vec<&'a str> {
    layout
        .lines
        .iter()
        .map(|l| &contents[l.source.clone()])
        .collect()
}

#[test]
fn a_capital_is_exactly_the_text_height_tall_on_the_first_line() {
    let layout = lay(&text("H"));
    assert_eq!(layout.glyphs.len(), 1);
    let g = &layout.glyphs[0];
    assert_eq!(g.id, glyph_of('H'));
    let top = glyph_contours(g.face, g.id, 1.0)
        .iter()
        .flatten()
        .map(|p| g.place(p.0, p.1).1)
        .fold(f64::MIN, f64::max);
    assert!(top.abs() < 1e-6, "the cap height lands on the top: {top}");
    assert!((layout.lines[0].baseline + H).abs() < 1e-9);
    assert!(layout.diagnostics.is_empty());
}

#[test]
fn advances_follow_the_font_and_the_width_factor() {
    let one = lay(&text("iii"));
    assert_eq!(one.glyphs.len(), 3);
    let xs: Vec<f64> = one.glyphs.iter().map(|g| g.x).collect();
    assert!(xs[0] == 0.0 && xs[1] > xs[0] && xs[2] > xs[1]);
    assert!((xs[2] - xs[1] - (xs[1] - xs[0])).abs() < 1e-9);

    let wide_style = TextStyle {
        width_factor: 2.0,
        ..TextStyle::standard()
    };
    let wide = layout_text(&text("iii"), &wide_style);
    assert!((wide.glyphs[1].x - 2.0 * xs[1]).abs() < 1e-9);
    assert!((wide.actual_width - 2.0 * one.actual_width).abs() < 1e-9);
    // `\W` multiplies the style's factor.
    let run = lay(&text("\\W2;iii"));
    assert!((run.glyphs[1].x - 2.0 * xs[1]).abs() < 1e-9);
}

#[test]
fn paragraph_breaks_make_lines_at_the_line_pitch() {
    let m = text("a\\Pb\\P\\Pc");
    let layout = lay(&m);
    assert_eq!(layout.lines.len(), 4);
    for (k, line) in layout.lines.iter().enumerate() {
        assert!((line.baseline + H + k as f64 * PITCH).abs() < 1e-9, "{k}");
    }
    assert_eq!(layout.lines[2].glyphs.len(), 0, "the empty line has none");
    assert_eq!(
        line_texts(&layout, &m.contents),
        ["a\\P", "b\\P", "\\P", "c"]
    );
    assert!(layout.actual_height > 3.0 * PITCH + H);
    assert_eq!(layout.frame_height, layout.actual_height);
}

#[test]
fn line_spacing_scales_or_fixes_the_pitch() {
    let mut m = text("a\\Pb");
    m.line_spacing.factor = 2.0;
    let l = lay(&m);
    assert!((l.lines[0].baseline - l.lines[1].baseline - 2.0 * PITCH).abs() < 1e-9);

    m.line_spacing.style = LineSpacingStyle::Exact;
    m.contents = "a\\H4;b\\Pc".into();
    m.line_spacing.factor = 1.0;
    let l = lay(&m);
    assert!(
        (l.lines[0].baseline - l.lines[1].baseline - PITCH).abs() < 1e-9,
        "exact ignores the taller run"
    );
    m.line_spacing.style = LineSpacingStyle::AtLeast;
    let l = lay(&m);
    assert!(
        l.lines[0].baseline - l.lines[1].baseline > PITCH,
        "at least grows"
    );
}

#[test]
fn heights_and_nesting_change_the_scale_of_a_run() {
    let l = lay(&text("a\\H2x;a{\\H8;a}a"));
    let scales: Vec<f64> = l.glyphs.iter().map(|g| g.scale_y).collect();
    assert!(
        (scales[1] / scales[0] - 2.0).abs() < 1e-9,
        "relative height"
    );
    assert!(
        (scales[2] / scales[0] - 4.0).abs() < 1e-9,
        "absolute height"
    );
    assert!(
        (scales[3] / scales[0] - 2.0).abs() < 1e-9,
        "the group ended"
    );
    // The line is as tall as its tallest run.
    assert!((l.lines[0].baseline + 8.0).abs() < 1e-9);
}

#[test]
fn slant_and_colour_come_from_the_controls() {
    let l = lay(&text("a\\Q30;a\\C1;a\\c255;a"));
    let shears: Vec<f64> = l.glyphs.iter().map(|g| g.shear).collect();
    assert_eq!(shears[0], 0.0);
    assert!((shears[1] - 30f64.to_radians().tan()).abs() < 1e-12);
    let colors: Vec<TextColor> = l.glyphs.iter().map(|g| g.color).collect();
    assert_eq!(
        colors,
        [
            TextColor::Entity,
            TextColor::Entity,
            TextColor::Aci(1),
            TextColor::Rgb(255)
        ]
    );
    let steep = lay(&text("\\Q89.9;a"));
    assert!(steep.glyphs[0].shear.is_finite() && steep.glyphs[0].shear < 12.0);
}

#[test]
fn text_wraps_at_spaces_and_trailing_spaces_hang() {
    let word = lay(&text("aaaa"));
    let w = word.actual_width;
    let m = wrapped("aaaa aaaa aaaa", 2.5 * w);
    let l = lay(&m);
    assert_eq!(line_texts(&l, &m.contents), ["aaaa aaaa ", "aaaa"]);
    for line in &l.lines {
        assert!(line.width <= 2.5 * w + 1e-9, "{line:?}");
    }
    assert_eq!(l.frame_width, 2.5 * w, "a wrapping text keeps its width");
    assert!(l.actual_width < l.frame_width);
}

#[test]
fn a_word_wider_than_the_width_overflows_without_stalling() {
    let m = wrapped("abcdefghijklmnop qr", 1.0);
    let l = lay(&m);
    assert_eq!(
        line_texts(&l, &m.contents),
        ["abcdefghijklmnop ", "qr"],
        "a word that does not fit runs past the width"
    );
    assert!(l.lines[0].width > 1.0);
    let tiny = lay(&wrapped("abc", 0.0001));
    assert_eq!(tiny.lines.len(), 1, "no break to take, so one line");
}

#[test]
fn breaks_fall_after_hyphens_and_around_ideographs() {
    let hyphen = lay(&text("well-known"));
    let narrow = wrapped("well-known", hyphen.actual_width * 0.7);
    assert_eq!(
        line_texts(&lay(&narrow), &narrow.contents),
        ["well-", "known"]
    );
    let cjk = wrapped("漢字漢字", 2.0);
    assert!(lay(&cjk).lines.len() > 1);
}

#[test]
fn tabs_run_to_the_next_multiple_of_the_default_interval() {
    let tab = DEFAULT_TAB * H;
    let l = lay(&text("a\tb\t\tc"));
    let xs: Vec<f64> = l.glyphs.iter().map(|g| g.x).collect();
    assert!((xs[1] - tab).abs() < 1e-9, "b is at the first stop");
    assert!((xs[2] - 3.0 * tab).abs() < 1e-9, "two tabs skip a stop");
    let exact = lay(&text("\ta"));
    assert!((exact.glyphs[0].x - tab).abs() < 1e-9);
}

#[test]
fn decorations_join_across_runs_with_the_same_look() {
    let m = text("\\Lab\\lcd \\Kef");
    let l = lay(&m);
    let under: Vec<&Decoration> = l
        .decorations
        .iter()
        .filter(|d| d.kind == DecorationKind::Underline)
        .collect();
    assert_eq!(under.len(), 1);
    assert!(under[0].x0 == 0.0 && under[0].x1 > 0.0);
    assert!(under[0].y < l.lines[0].baseline);
    let strike: Vec<&Decoration> = l
        .decorations
        .iter()
        .filter(|d| d.kind == DecorationKind::Strike)
        .collect();
    assert_eq!(strike.len(), 1);
    assert!(strike[0].y > l.lines[0].baseline);
    let both = lay(&text("\\L\\Oab"));
    assert_eq!(both.decorations.len(), 2);
}

#[test]
fn what_is_not_laid_out_is_reported() {
    let issues = |contents: &str| -> Vec<LayoutIssue> {
        lay(&text(contents))
            .diagnostics
            .into_iter()
            .map(|d| d.issue)
            .collect()
    };
    assert_eq!(issues("%<\\AcVar Date>%"), [LayoutIssue::FieldNotEvaluated]);
    assert!(issues("\\fArial;a").contains(&LayoutIssue::FontSubstituted));
    assert!(issues("\\pxqd;a").contains(&LayoutIssue::Skipped(OpaqueReason::ParagraphControl)));
    assert!(issues("a").is_empty());
    assert!(
        issues("a\\S1/2;b").is_empty(),
        "a stack is laid out, not reported"
    );

    let styled = TextStyle {
        font: FontDescriptor::Shx {
            file_name: "txt.shx".into(),
        },
        ..TextStyle::standard()
    };
    let l = layout_text(&text("a"), &styled);
    assert_eq!(l.diagnostics[0].issue, LayoutIssue::FontSubstituted);
    assert_eq!(l.glyphs.len(), 1, "still drawn, in the bundled font");

    let mut flowing = text("a");
    flowing.flow = MTextFlow::TopToBottom;
    assert!(lay(&flowing)
        .diagnostics
        .iter()
        .any(|d| d.issue == LayoutIssue::FlowNotSupported));

    let missing = lay(&text("\u{10FFFD}"));
    assert!(missing
        .diagnostics
        .iter()
        .any(|d| d.issue == LayoutIssue::MissingGlyph));
}

#[test]
fn unreadable_contents_draw_nothing_and_say_why() {
    let big = text(&"a".repeat((1 << 20) + 1));
    let l = lay(&big);
    assert!(l.glyphs.is_empty());
    assert!(matches!(
        l.diagnostics[0].issue,
        LayoutIssue::Unreadable(TextError::ResourceLimit {
            resource: TextResource::ContentBytes,
            ..
        })
    ));
}

#[test]
fn empty_text_is_one_empty_line_with_a_frame() {
    let l = lay(&text(""));
    assert_eq!(l.lines.len(), 1);
    assert!(l.glyphs.is_empty());
    assert!((l.frame_width - 0.6 * H).abs() < 1e-9);
    assert!(l.frame_height > H);
}

#[test]
fn right_to_left_text_shapes_and_gets_glyphs() {
    let l = lay(&text("مرحبا"));
    assert!(!l.glyphs.is_empty());
    assert!(l.actual_width > 0.0);
    let hebrew = lay(&text("שלום"));
    assert_eq!(hebrew.glyphs.len(), 4);
}

#[test]
fn outlines_are_closed_contours_and_spaces_have_none() {
    let h = glyph_contours(FontFace::Regular, glyph_of('H'), 1.0);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].len(), 12, "an H is twelve straight edges");
    let o = glyph_contours(FontFace::Regular, glyph_of('O'), 1.0);
    assert_eq!(o.len(), 2, "outside and inside");
    let coarse: usize = glyph_contours(FontFace::Regular, glyph_of('O'), 50.0)
        .iter()
        .map(Vec::len)
        .sum();
    let fine: usize = glyph_contours(FontFace::Regular, glyph_of('O'), 0.5)
        .iter()
        .map(Vec::len)
        .sum();
    assert!(fine > coarse, "a finer tolerance uses more points");
    assert!(glyph_contours(FontFace::Regular, glyph_of(' '), 1.0).is_empty());
}

#[test]
fn moving_text_keeps_its_remembered_layout_and_changing_it_does_not() {
    let style = TextStyle::standard();
    let a = text("same");
    let first = a.layout(&style);
    let moved = a.translated(5.0, 5.0).rotated(Point::new(0.0, 0.0), 1.0);
    assert!(Rc::ptr_eq(&first, &moved.layout(&style)));
    let edited = text("same!");
    assert!(!Rc::ptr_eq(&first, &edited.layout(&style)));
    assert_eq!(*first, lay(&a), "remembering changes no answer");
}

#[test]
fn hostile_contents_always_lay_out() {
    const PIECES: &[&str] = &[
        "\\P", "\\L", "\\H3;", "\\H0.5x;", "{", "}", " ", "\t", "ab", "W", "-", "漢", "م",
        "\\S1/2;", "\\pxi;", "%<x>%", "\\Q80;", "é", "\\~",
    ];
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..300 {
        let count = (next() % 30) as usize;
        let contents: String = (0..count)
            .map(|_| PIECES[(next() % PIECES.len() as u64) as usize])
            .collect();
        let width = [0.0, 0.5, 3.0, 20.0][round % 4];
        let m = wrapped(&contents, width);
        let l = lay(&m);
        assert!(!l.lines.is_empty(), "{contents:?}");
        let mut end = 0;
        for line in &l.lines {
            assert_eq!(line.source.start, end, "{contents:?}");
            assert!(line.source.end >= line.source.start);
            assert!(line.glyphs.end <= l.glyphs.len());
            assert!(line.width.is_finite());
            end = line.source.end;
        }
        assert_eq!(end, contents.len(), "lines tile the contents: {contents:?}");
        assert!(l.frame_width.is_finite() && l.frame_height.is_finite());
    }
}

#[test]
fn a_word_wider_than_the_width_runs_past_the_frame_and_the_layout_says_so() {
    let l = lay(&wrapped("abcdefghij", 3.0));
    assert!(l.frame_width <= 3.0 + 1e-9);
    assert!(l.actual_width > l.frame_width, "an editor must cover this");
}
