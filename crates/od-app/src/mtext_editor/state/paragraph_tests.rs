//! Tests of paragraph formatting in the editor.

use od_core::text::ParagraphEdit;

use super::tests::{caret_at, editor, editor_wrapped};

fn left(v: f64) -> ParagraphEdit {
    ParagraphEdit {
        left_indent: Some(v),
        ..ParagraphEdit::default()
    }
}

#[test]
fn an_indent_is_one_undoable_step_and_the_caret_stays_on_its_letter() {
    let mut e = editor("hello");
    caret_at(&mut e, 2);
    assert!(e.set_paragraph(&left(4.0), false, 0.0));
    assert_eq!(e.contents(), r"\pxl4;hello");
    assert_eq!(e.paragraph().style.left_indent, 4.0);
    assert_eq!(e.selected_range(), 8..8);
    assert!(!e.set_paragraph(&left(4.0), false, 1.0), "nothing changes");
    assert!(e.undo());
    assert_eq!(e.contents(), "hello");
    assert_eq!(e.selected_range(), 2..2);
    assert!(!e.is_changed());
}

#[test]
fn a_drag_of_a_marker_is_one_undo_step() {
    let mut e = editor("hello");
    for (i, v) in [1.0, 2.0, 3.0].into_iter().enumerate() {
        assert!(e.set_paragraph(&left(v), true, 0.1 * i as f64));
    }
    assert_eq!(e.contents(), r"\pxl3;hello");
    assert!(e.undo());
    assert_eq!(e.contents(), "hello");
}

#[test]
fn a_protected_paragraph_is_refused_with_a_reason() {
    let mut e = editor(r"\pxqd;hello");
    assert!(e.paragraph().protected);
    assert!(!e.set_paragraph(&left(4.0), false, 0.0));
    assert!(e.notice.is_some());
    assert_eq!(e.contents(), r"\pxqd;hello");
}

#[test]
fn a_new_paragraph_inherits_the_indents_of_the_one_it_split_from() {
    let mut e = editor(r"\pxi-4,l4;one");
    assert!(e.insert("\n", 0.0));
    assert_eq!(e.contents(), r"\pxi-4,l4;one\P", "no second control needed");
    assert_eq!(e.paragraph().style.left_indent, 4.0);
    assert!(e.insert("two", 0.1));
    assert_eq!(e.paragraph().style.first_indent, -4.0);
    assert!(e.undo());
    assert!(!e.can_undo(), "the break and the typing are one step");
    assert_eq!(e.contents(), r"\pxi-4,l4;one");
}

#[test]
fn changing_one_paragraph_leaves_the_ones_after_it_as_they_were() {
    let mut e = editor(r"\pxl2;one\Ptwo\Pthree");
    caret_at(&mut e, 12);
    assert_eq!(e.paragraph().style.left_indent, 2.0);
    assert!(e.set_paragraph(&left(5.0), false, 0.0));
    assert_eq!(e.contents(), r"\pxl2;one\P\pxl5;two\P\pxl2;three");
    assert!(e.undo());
    assert_eq!(e.contents(), r"\pxl2;one\Ptwo\Pthree");
}

#[test]
fn alignment_is_set_from_the_editor() {
    use od_core::text::ParagraphAlignment;
    let mut e = editor("hello");
    let centre = ParagraphEdit {
        alignment: Some(ParagraphAlignment::Center),
        ..ParagraphEdit::default()
    };
    assert!(e.set_paragraph(&centre, false, 0.0));
    assert_eq!(e.contents(), r"\pxqc;hello");
    assert_eq!(e.paragraph().style.alignment, ParagraphAlignment::Center);
}

#[test]
fn the_width_changes_the_wrap_and_undo_brings_it_back() {
    let mut e = super::tests::editor_wrapped("one two three four", 40.0);
    assert_eq!(e.layout().lines.len(), 1);
    assert!(e.set_width(8.0, false, 0.0));
    assert!(e.layout().lines.len() > 1, "narrower, so it wraps");
    assert_eq!(e.text().reference_width, 8.0);
    assert!(e.is_changed());
    assert!(!e.set_width(8.0, false, 1.0), "the same width is no step");
    assert!(!e.set_width(0.0, false, 1.0), "a width must be positive");
    assert!(e.undo());
    assert_eq!(e.text().reference_width, 40.0);
    assert_eq!(e.layout().lines.len(), 1);
    assert!(!e.is_changed());
    assert!(e.redo());
    assert_eq!(e.text().reference_width, 8.0);
}

#[test]
fn dragging_the_width_is_one_step() {
    let mut e = super::tests::editor_wrapped("one two three four", 40.0);
    for (i, w) in [30.0, 20.0, 10.0].into_iter().enumerate() {
        assert!(e.set_width(w, true, 0.1 * i as f64));
    }
    assert!(e.undo());
    assert_eq!(e.text().reference_width, 40.0);
}

#[test]
fn enter_continues_a_list_and_ends_it_from_an_empty_item_in_one_step_each() {
    use od_core::text::ListKind;
    let mut e = editor("one");
    assert!(e.set_list(Some(ListKind::Number), 0.0));
    e.move_caret(super::Move::DocEnd, false);
    assert!(e.insert("\n", 1.0));
    assert!(e.contents().ends_with("one\\P2.\t"), "{}", e.contents());
    assert_eq!(e.list_kind(), Some(ListKind::Number));
    assert!(e.insert("two", 1.1));
    assert!(e.insert("\n", 2.0));
    assert!(e.insert("\n", 3.0), "an empty item ends the list");
    assert_eq!(e.list_kind(), None);
    assert_eq!(e.paragraph().style.left_indent, 0.0);
    assert!(e.undo());
    assert_eq!(e.list_kind(), Some(ListKind::Number));
}

#[test]
fn a_list_is_made_toggled_and_undone_in_one_step_each() {
    use od_core::text::ListKind;
    let mut e = editor(r"one\Ptwo");
    e.select_all();
    assert_eq!(e.list_kind(), None);
    assert!(e.set_list(Some(ListKind::Number), 0.0));
    // Text height 2 in the test editor: a hanging indent of three heights.
    assert_eq!(e.contents(), "\\pxi-6,l6,t6;1.\tone\\P2.\ttwo");
    assert_eq!(e.list_kind(), Some(ListKind::Number));
    assert!(e.set_list(None, 1.0));
    assert_eq!(e.contents(), r"one\Ptwo");
    assert!(e.undo());
    assert_eq!(e.list_kind(), Some(ListKind::Number));
    assert!(e.undo());
    assert_eq!(e.contents(), r"one\Ptwo");
    assert!(!e.set_list(None, 2.0), "already not a list");
}

#[test]
fn the_box_keeps_the_height_it_was_dragged_to_as_the_text_changes() {
    let mut e = editor_wrapped("one", 40.0);
    let line = e.box_height();
    assert!(line > 0.0);
    e.set_box_height(line * 4.0);
    assert_eq!(e.box_height(), line * 4.0);
    // Typing and deleting leave it alone.
    e.insert("x", 0.0);
    assert_eq!(e.box_height(), line * 4.0);
    e.select_all();
    e.insert("", 1.0);
    e.set_box_height(f64::NAN);
    assert_eq!(e.box_height(), line * 4.0);
    // It cannot be dragged shorter than the text.
    e.set_box_height(0.0);
    assert!(e.box_height() > 0.0);
}
