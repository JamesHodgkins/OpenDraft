//! Tests of paragraph alignment in the layout, from the second AutoCAD
//! sample (`\pxqr;`, `\pxqc;` and `\pxqj;` on text of height 1).

use super::tests::lay;
use super::*;
use crate::text_style::TextStyleId;
use crate::Point;

fn sample(contents: &str, width: f64) -> TextLayout {
    let mut m = MText::new(Point::new(0.0, 0.0), 1.0, contents, TextStyleId::STANDARD);
    m.reference_width = width;
    lay(&m)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// The right edge of the text on `line`.
fn right_edge(l: &TextLayout, line: usize) -> f64 {
    l.lines[line].width
}

#[test]
fn right_aligned_lines_end_at_the_width_and_the_next_paragraph_keeps_it() {
    let src = r"\pxqr;THIS IS SOME RIGHT ALIGNED TEXT\PTHE TEXT IS ALIGNED";
    let l = sample(src, 38.58);
    assert_eq!(l.lines.len(), 2);
    assert!(near(right_edge(&l, 0), 38.58), "{}", right_edge(&l, 0));
    assert!(near(right_edge(&l, 1), 38.58), "the control holds on");
    // The caret at the end of the line is at the edge too.
    let end = l.carets[l.lines[0].carets.end - 1].x;
    assert!(near(end, 38.58), "{end}");
    // Plain text is left as it was.
    let plain = sample("THIS IS SOME RIGHT ALIGNED TEXT", 38.58);
    assert!(near(plain.glyphs[0].x, 0.0));
    assert!(right_edge(&plain, 0) < 38.0);
}

#[test]
fn centred_lines_sit_in_the_middle_of_the_width() {
    let text = "THIS IS SOME CENTRED TEXT";
    let plain = sample(text, 38.58);
    let centred = sample(&format!(r"\pxqc;{text}"), 38.58);
    let w = right_edge(&plain, 0);
    assert!(near(right_edge(&centred, 0), w + (38.58 - w) / 2.0));
    assert!(near(centred.glyphs[0].x, (38.58 - w) / 2.0,));
    let (a, b) = (centred.glyphs[0].x, right_edge(&centred, 0));
    assert!(near(a + b, 38.58), "equal room each side: {a} and {b}");
}

#[test]
fn a_justified_paragraph_fills_every_line_but_its_last() {
    let text = "ONE TWO THREE FOUR FIVE SIX SEVEN EIGHT NINE TEN ELEVEN TWELVE";
    let l = sample(&format!(r"\pxqj;{text}"), 20.0);
    assert!(l.lines.len() >= 3);
    for line in 0..l.lines.len() - 1 {
        assert!(
            near(right_edge(&l, line), 20.0),
            "line {line}: {}",
            right_edge(&l, line)
        );
        assert!(near(l.glyphs[l.lines[line].glyphs.start].x, 0.0));
    }
    let last = l.lines.len() - 1;
    assert!(right_edge(&l, last) < 20.0, "the last line stays ragged");
    // Words stay in order and apart.
    let xs: Vec<f64> = l.glyphs.iter().map(|g| g.x).collect();
    for line in &l.lines {
        let run = &xs[line.glyphs.clone()];
        assert!(run.windows(2).all(|w| w[1] >= w[0] - 1e-9));
    }
}

#[test]
fn a_justified_line_ended_by_a_break_is_not_stretched() {
    let l = sample(r"\pxqj;SHORT LINE\PANOTHER", 30.0);
    assert!(right_edge(&l, 0) < 30.0);
    assert!(near(l.glyphs[0].x, 0.0));
}

#[test]
fn without_a_width_lines_align_to_the_widest_line() {
    let l = sample(r"\pxqr;LONG LONG LONG\PSHORT", 0.0);
    assert!(near(right_edge(&l, 0), right_edge(&l, 1)));
    assert!(near(l.actual_width, right_edge(&l, 0)));
}

#[test]
fn alignment_and_indents_work_together() {
    // An indented, right aligned line still ends at the width.
    let l = sample(r"\pxi2,qr;TEXT", 20.0);
    assert!(near(right_edge(&l, 0), 20.0));
    assert!(l.glyphs[0].x > 2.0);
}
