use crate::mtext::MText;
use crate::text::{layout_text, TextLayout};
use crate::text_style::{TextStyle, TextStyleId};
use crate::Point;

fn lay(contents: &str, width: f64) -> TextLayout {
    let mut m = MText::new(Point::new(0.0, 0.0), 2.0, contents, TextStyleId::STANDARD);
    m.reference_width = width;
    layout_text(&m, &TextStyle::standard())
}

fn sources(layout: &TextLayout) -> Vec<usize> {
    layout.carets.iter().map(|c| c.source).collect()
}

#[test]
fn a_line_has_a_caret_before_each_cluster_and_one_at_the_end() {
    let l = lay("abc", 0.0);
    assert_eq!(sources(&l), [0, 1, 2, 3]);
    let xs: Vec<f64> = l.carets.iter().map(|c| c.x).collect();
    assert_eq!(xs[0], 0.0);
    assert!(xs.windows(2).all(|w| w[1] > w[0]));
    assert!((xs[3] - l.lines[0].width).abs() < 1e-9);
    assert_eq!(l.lines[0].carets, 0..4);
    assert_eq!(l.carets[1].prev, Some('a'));
    assert_eq!(l.carets[1].next, Some('b'));
    assert_eq!(l.carets[3].next, None);
    assert_eq!(l.carets[0].prev, None);
    assert_eq!(lay("", 0.0).carets.len(), 1, "an empty text has one caret");
}

#[test]
fn controls_have_no_carets_of_their_own() {
    // The caret between a and B sits after the underline control.
    let l = lay("a\\LB\\l", 0.0);
    assert_eq!(sources(&l), [0, 3, 6]);
    // A stack is one unit.
    let l = lay("x\\S1/2;y", 0.0);
    assert_eq!(sources(&l), [0, 1, 7, 8]);
    // A multi-byte letter and an escape are single steps.
    let l = lay("é\\\\", 0.0);
    assert_eq!(sources(&l), [0, 2, 4]);
}

#[test]
fn a_paragraph_break_ends_a_line_before_it_and_the_next_starts_after() {
    let l = lay("ab\\Pcd", 0.0);
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[0].carets, 0..3);
    assert_eq!(sources(&l), [0, 1, 2, 4, 5, 6]);
    assert_eq!(l.step(2, true), 3, "past the break to the next line");
    assert_eq!(l.step(3, false), 2);
    let empty = lay("\\P", 0.0);
    assert_eq!(sources(&empty), [0, 2]);
}

#[test]
fn a_wrap_gives_two_carets_at_one_byte_and_they_are_told_apart() {
    let one = lay("aaaa", 0.0);
    let l = lay("aaaa aaaa", 1.3 * one.actual_width);
    assert_eq!(l.lines.len(), 2);
    let end = l.lines[0].carets.end - 1;
    let start = l.lines[1].carets.start;
    assert_eq!(l.carets[end].source, l.carets[start].source);
    assert_eq!(l.carets[end].source, 5);
    assert_eq!(l.caret_at(5, true), Some(end));
    assert_eq!(l.caret_at(5, false), Some(start));
    assert!(l.is_upstream(end) && !l.is_upstream(start));
    // Stepping right from before the break does not stop at the twin.
    let before = end - 1;
    assert_eq!(l.step(before, true), end);
    assert_eq!(l.carets[l.step(before, true)].source, 5);
    assert_eq!(
        l.step(end, true),
        start + 1,
        "from the end of the line, to the next byte"
    );
    assert_eq!(l.step(start + 1, false), start);
    assert_eq!(
        l.step(start, false),
        end - 1,
        "and past the twin going back"
    );
}

#[test]
fn a_byte_with_no_caret_gets_the_one_it_stands_next_to() {
    let l = lay("a\\LB", 0.0);
    // Byte 2 is inside the control after the `a`: the caret after the `a`.
    assert_eq!(l.caret_at(2, false), Some(1));
    assert_eq!(l.caret_at(3, false), Some(1));
    assert_eq!(l.caret_at(99, false), Some(2), "beyond the end: the last");
    assert_eq!(TextLayout::default().caret_at(0, false), None);
}

#[test]
fn clicking_finds_the_nearest_line_then_the_nearest_stop() {
    let l = lay("abc\\Pdef", 0.0);
    let (x1, _, _) = l.caret_rect(1).unwrap();
    let (x2, _, _) = l.caret_rect(2).unwrap();
    let mid = (x1 + x2) / 2.0;
    let first = l.lines[0].baseline;
    let second = l.lines[1].baseline;
    assert_eq!(l.nearest_caret(x1 + 0.01, first + 1.0), Some(1));
    assert_eq!(l.nearest_caret(mid + 0.01, first), Some(2));
    assert_eq!(l.nearest_caret(-50.0, first), Some(0));
    assert_eq!(
        l.nearest_caret(500.0, first),
        Some(3),
        "past the end of the line"
    );
    assert_eq!(
        l.nearest_caret(x1, second),
        Some(l.lines[1].carets.start + 1)
    );
    assert_eq!(
        l.nearest_caret(x1, -500.0),
        Some(l.lines[1].carets.start + 1),
        "below everything"
    );
    assert_eq!(TextLayout::default().nearest_caret(0.0, 0.0), None);
}

#[test]
fn the_caret_stands_on_its_line_and_reaches_above_and_below_the_text() {
    let l = lay("ab\\P\\H4;cd", 0.0);
    let (_, bottom0, top0) = l.caret_rect(0).unwrap();
    assert!(bottom0 < l.lines[0].baseline && top0 > l.lines[0].baseline + 2.0);
    let second = l.lines[1].carets.start;
    let (_, bottom1, top1) = l.caret_rect(second).unwrap();
    assert!(
        top1 - bottom1 > 2.0 * (top0 - bottom0) * 0.99,
        "a taller line has a taller caret"
    );
}

#[test]
fn words_are_jumped_by_their_class() {
    let l = lay("one two,three", 0.0);
    // Stops: o0 n1 e2 ' '3 t4 w5 o6 ,7 t8 ...
    assert_eq!(l.word_step(0, true), 4, "over the word and its space");
    assert_eq!(l.word_step(4, true), 7, "to the comma");
    assert_eq!(l.word_step(7, true), 8, "over the comma");
    assert_eq!(l.word_step(8, true), 13, "to the end");
    assert_eq!(l.word_step(13, false), 8);
    assert_eq!(l.word_step(8, false), 7);
    assert_eq!(l.word_step(7, false), 4);
    assert_eq!(l.word_step(4, false), 0);
    assert_eq!(l.word_step(0, false), 0);
    let two = lay("ab\\Pcd", 0.0);
    assert_eq!(two.word_step(2, true), 3, "a line end is a single step");
    assert_eq!(two.word_step(3, false), 2);
}

#[test]
fn home_end_and_up_down() {
    let l = lay("abc\\Pde\\Pfghij", 0.0);
    assert_eq!(l.line_start(2), 0);
    assert_eq!(l.line_end(1), 3);
    let (x, _, _) = l.caret_rect(3).unwrap();
    let down = l.vertical(3, x, false).unwrap();
    assert_eq!(l.carets[down].line, 1);
    assert_eq!(down, l.lines[1].carets.end - 1, "the short line's end");
    let down2 = l.vertical(down, x, false).unwrap();
    assert_eq!(l.carets[down2].line, 2);
    let off = |i: usize| (l.carets[i].x - x).abs();
    assert!(l.lines[2].carets.clone().all(|i| off(down2) <= off(i)));
    assert_eq!(l.vertical(down2, x, false), None, "no line below the last");
    assert_eq!(l.vertical(0, 0.0, true), None, "none above the first");
    assert_eq!(l.carets[l.vertical(down2, x, true).unwrap()].line, 1);
}

#[test]
fn a_selection_is_a_box_per_line_and_marks_paragraph_breaks_only() {
    let l = lay("abc\\Pdef", 0.0);
    assert!(l.selection_rects(2, 2).is_empty());
    let one = l.selection_rects(1, 3);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].x0, l.carets[1].x);
    assert_eq!(
        one[0].x1, l.carets[3].x,
        "ends at the caret when the selection stops there"
    );
    let two = l.selection_rects(5, 1);
    assert_eq!(two.len(), 2, "either order works");
    assert!(
        two[0].x1 > l.carets[3].x,
        "the break at the end of the first line is marked"
    );
    assert_eq!(two[1].x0, 0.0);
    assert_eq!(two[1].x1, l.carets[5].x);
    assert!(
        two[0].top > two[0].bottom && two[1].top < two[0].bottom,
        "lower line, lower box"
    );

    let one_word = lay("aaaa", 0.0);
    let wrapped = lay("aaaa aaaa", 1.3 * one_word.actual_width);
    let rects = wrapped.selection_rects(0, wrapped.carets.len() - 1);
    assert_eq!(rects.len(), 2);
    assert_eq!(
        rects[0].x1,
        wrapped.carets[wrapped.lines[0].carets.end - 1].x,
        "a wrap is not marked"
    );
}

#[test]
fn carets_exist_for_hostile_text_and_tile_the_contents() {
    const PIECES: &[&str] = &[
        "\\P", "\\L", "\\H3;", "{", "}", " ", "\t", "ab", "-", "漢", "م", "\\S1/2;", "\\pxi;",
        "%<x>%", "é", "\\~", "\\\\",
    ];
    let mut state = 0x1234_5678_9abc_def1_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..300 {
        let count = (next() % 25) as usize;
        let contents: String = (0..count)
            .map(|_| PIECES[(next() % PIECES.len() as u64) as usize])
            .collect();
        let l = lay(&contents, [0.0, 2.0, 8.0][round % 3]);
        for (k, line) in l.lines.iter().enumerate() {
            assert!(!line.carets.is_empty(), "{contents:?}");
            for c in &l.carets[line.carets.clone()] {
                assert_eq!(c.line, k);
                assert!(
                    c.source <= contents.len() && contents.is_char_boundary(c.source),
                    "{contents:?}"
                );
                assert!(c.x.is_finite());
            }
        }
        // Every motion lands on a real caret.
        for i in 0..l.carets.len() {
            for forward in [true, false] {
                assert!(l.step(i, forward) < l.carets.len());
                assert!(l.word_step(i, forward) < l.carets.len());
            }
            assert!(l.line_start(i) <= i && i <= l.line_end(i));
        }
    }
}

#[test]
fn a_word_is_the_run_of_one_kind_around_a_caret() {
    let l = lay("one two,x", 0.0);
    // Stops: o0 n1 e2 ' '3 t4 w5 o6 ,7 x8 end9
    assert_eq!(l.word_at(1), (0, 3));
    assert_eq!(l.word_at(0), (0, 3));
    assert_eq!(l.word_at(3), (3, 4), "the space between");
    assert_eq!(l.word_at(5), (4, 7));
    assert_eq!(l.word_at(7), (7, 8), "the comma");
    assert_eq!(l.word_at(9), (8, 9), "at the end: the word before");
    assert_eq!(lay("", 0.0).word_at(0), (0, 0));
    let two = lay("ab\\Pcd", 0.0);
    assert_eq!(two.word_at(1), (0, 2), "a word stops at the line");
}

#[test]
fn a_byte_inside_a_control_after_text_gets_the_caret_after_that_text() {
    // `}` closes a group after the last letter: the end of a selection that
    // takes the group must include the letter.
    let src = r"{\T2;HELLO}";
    let l = lay(src, 0.0);
    let end = l.caret_at(src.len() - 1, false).unwrap();
    assert_eq!(end, l.carets.len() - 1, "the end of the line, after the O");
    // A control between two letters sits at the second one's caret.
    let mid = r"AB{\Lcd}";
    let m = lay(mid, 0.0);
    let after_b = m.caret_at(3, false).unwrap();
    assert_eq!(m.carets[after_b].source, 5, "before the c");
    // A control before the first letter belongs to the first caret.
    assert_eq!(l.caret_at(2, false), Some(0));
}
