//! Tests of paragraph indents and tab stops in the layout. The contents and
//! widths come from the AutoCAD sample in `private_material/MText Example.dxf`
//! (text height 1), whose expected look is `MText Example.png`.

use super::tests::lay;
use super::*;
use crate::text_style::TextStyleId;
use crate::Point;

fn sample(contents: &str, width: f64) -> TextLayout {
    let mut m = MText::new(Point::new(0.0, 0.0), 1.0, contents, TextStyleId::STANDARD);
    m.reference_width = width;
    lay(&m)
}

/// The x of the first glyph on each line.
fn line_starts(layout: &TextLayout) -> Vec<f64> {
    layout
        .lines
        .iter()
        .filter_map(|l| layout.glyphs.get(l.glyphs.start).map(|g| g.x))
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_tab_stop_puts_the_text_at_that_distance() {
    // `\pxt2;` then a tab: the text starts 2 units in, not at the 4-unit default.
    let l = sample("\\pxt2;\tTHIS IS SOME TEXT WITH A LEFT TAB INDENT", 40.8);
    assert!(near(l.glyphs[0].x, 2.0), "{}", l.glyphs[0].x);
    assert_eq!(l.lines.len(), 1);
}

#[test]
fn tabs_past_the_custom_stops_use_the_default_grid() {
    let l = sample("\\pxt2;\ta\tb\tc", 0.0);
    let xs: Vec<f64> = l.glyphs.iter().map(|g| g.x).collect();
    assert!(near(xs[0], 2.0));
    assert!(near(xs[1], 4.0), "next default stop after 2 + a glyph");
    assert!(near(xs[2], 8.0));
}

#[test]
fn a_first_line_indent_moves_only_the_first_line() {
    let l = sample("\\pxi4;THIS IS SOME TEXT WITH FIRST LINE INDENTATION", 28.6);
    let starts = line_starts(&l);
    assert!(starts.len() >= 2, "it wraps");
    assert!(near(starts[0], 4.0));
    assert!(starts[1..].iter().all(|x| near(*x, 0.0)));
}

#[test]
fn a_hanging_indent_leaves_the_first_line_and_moves_the_rest() {
    let l = sample(
        "\\pxi-4,l4;THIS IS SOME TEXT WITH HANGING INDENTATION. THIS TEXT IS HERE JUST TO INCREASE THE WORD COUNT",
        31.7,
    );
    let starts = line_starts(&l);
    assert!(starts.len() >= 2, "it wraps");
    assert!(near(starts[0], 0.0));
    assert!(starts[1..].iter().all(|x| near(*x, 4.0)));
    assert!(l.lines.iter().all(|line| line.width <= 31.7 + 1e-6));
}

#[test]
fn a_paragraph_control_governs_its_paragraph_wherever_it_sits_and_those_after_it() {
    let l = sample("a\\pxi3;b\\Pc\\P\\px;d", 0.0);
    let xs: Vec<f64> = l.glyphs.iter().map(|g| g.x).collect();
    assert!(near(xs[0], 3.0), "the control after the `a` still applies");
    assert!(xs[1] > xs[0]);
    assert!(near(xs[2], 3.0), "the next paragraph keeps it");
    assert!(near(xs[3], 0.0), "until another control resets it");
}

#[test]
fn a_numbered_list_hangs_as_in_the_second_autocad_sample() {
    // Only the first item carries the control; the tab runs to the stop at 3.
    let src = "LIST\\P\\pxi-3,l3,t3;1.\tITEM ONE\\P2.\tITEM TWO\\P3.\tITEM THREE";
    let l = sample(src, 38.58);
    assert_eq!(l.lines.len(), 4);
    for (line, item) in l.lines[1..]
        .iter()
        .zip(["ITEM ONE", "ITEM TWO", "ITEM THREE"])
    {
        let first = l.glyphs[line.glyphs.start].x;
        assert!(near(first, 0.0), "the number is at the margin: {first}");
        // After the tab, the text starts at the stop.
        let text_at = src.find(item).unwrap();
        let g = l.glyphs[line.glyphs.clone()]
            .iter()
            .find(|g| g.source.start >= text_at)
            .unwrap();
        assert!(near(g.x, 3.0), "{item}: {}", g.x);
    }
}

#[test]
fn plain_text_is_unchanged_and_the_caret_follows_the_indent() {
    let l = sample("abc", 0.0);
    assert!(near(l.glyphs[0].x, 0.0));
    let indented = sample("\\pxi4;abc", 0.0);
    assert!(near(indented.carets[0].x, 4.0));
    assert_eq!(indented.carets[0].source, 6, "after the control");
}

#[test]
fn a_paragraph_control_where_a_line_wraps_leaves_every_caret_inside_the_text() {
    // Found by the editor's stress test: the control began the second line
    // and its position was taken for the end of the first.
    let bs = '\u{5c}';
    for width in [0.5, 2.0, 5.0, 10.0] {
        let src = format!(" {bs}pxqj;{bs}{bs};A. {bs}pxi2; B {bs}pxl3;C");
        let l = sample(&src, width);
        for stop in &l.carets {
            assert!(stop.source <= src.len(), "width {width}: {}", stop.source);
        }
        for line in &l.lines {
            assert!(line.source.end <= src.len(), "width {width}");
        }
    }
}
