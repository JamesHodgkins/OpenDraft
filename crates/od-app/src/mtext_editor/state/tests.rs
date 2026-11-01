use od_core::{MText, Point, TextStyle, TextStyleId};

use super::*;

pub(super) fn editor(contents: &str) -> MTextEditor {
    editor_wrapped(contents, 0.0)
}

pub(super) fn editor_wrapped(contents: &str, width: f64) -> MTextEditor {
    let mut text = MText::new(Point::new(0.0, 0.0), 2.0, contents, TextStyleId::STANDARD);
    text.reference_width = width;
    MTextEditor::new(text, TextStyle::standard())
}

/// Put the caret at byte `source` with nothing selected.
pub(super) fn caret_at(e: &mut MTextEditor, source: usize) {
    e.anchor = Pos {
        source,
        upstream: false,
    };
    e.active = e.anchor;
}

pub(super) fn selected(e: &MTextEditor) -> &str {
    &e.contents()[e.selected_range()]
}

#[test]
fn a_new_editor_has_the_caret_at_the_end_and_nothing_changed() {
    let e = editor("abc");
    assert_eq!(e.selected_range(), 3..3);
    assert!(!e.has_selection() && !e.is_changed());
    assert!(!e.can_undo() && !e.can_redo());
    assert_eq!(e.caret(), 3);
}

#[test]
fn typing_inserts_at_the_caret_and_escapes_what_needs_it() {
    let mut e = editor("");
    assert!(e.insert("ab", 0.0));
    assert!(e.insert("{c}", 0.1));
    assert!(e.insert("\n", 0.2));
    assert!(e.insert("\\", 0.3));
    assert_eq!(e.contents(), "ab\\{c\\}\\P\\\\");
    assert_eq!(e.selected_range().start, e.contents().len());
    assert!(!e.insert("", 0.4));
    caret_at(&mut e, 2);
    e.insert("X", 1.0);
    assert_eq!(e.contents(), "abX\\{c\\}\\P\\\\");
    assert!(e.is_changed());
}

#[test]
fn typing_over_a_selection_replaces_it_in_one_undoable_step() {
    let mut e = editor("hello world");
    e.select_all();
    assert_eq!(selected(&e), "hello world");
    e.insert("x", 0.0);
    assert_eq!(e.contents(), "x");
    assert!(e.undo());
    assert_eq!(e.contents(), "hello world");
    assert_eq!(selected(&e), "hello world", "the selection comes back");
    assert!(e.redo());
    assert_eq!(e.contents(), "x");
    assert!(!e.redo());
}

#[test]
fn backspace_and_delete_remove_one_cluster_whole() {
    let mut e = editor("aé\\\\b");
    assert!(e.backspace(false, 0.0));
    assert_eq!(e.contents(), "aé\\\\");
    assert!(e.backspace(false, 0.1), "an escaped backslash goes whole");
    assert_eq!(e.contents(), "aé");
    assert!(e.backspace(false, 0.2));
    assert_eq!(e.contents(), "a");
    caret_at(&mut e, 0);
    assert!(!e.backspace(false, 0.3), "nothing before the start");
    assert!(e.delete(false, 0.4));
    assert_eq!(e.contents(), "");
    assert!(!e.delete(false, 0.5), "nothing after the end");
}

#[test]
fn deleting_a_word_or_a_paragraph_break() {
    let mut e = editor("one two three");
    e.backspace(true, 0.0);
    assert_eq!(e.contents(), "one two ");
    e.backspace(true, 1.0);
    assert_eq!(e.contents(), "one ");
    caret_at(&mut e, 0);
    e.delete(true, 2.0);
    assert_eq!(e.contents(), "");

    let mut e = editor("a\\Pb");
    caret_at(&mut e, 3);
    assert!(e.backspace(false, 0.0));
    assert_eq!(
        e.contents(),
        "ab",
        "the break between the lines is one backspace"
    );
    let mut e = editor("a\\Pb");
    caret_at(&mut e, 1);
    assert!(e.delete(false, 0.0));
    assert_eq!(e.contents(), "ab");
}

#[test]
fn backspacing_at_a_control_leaves_it_alone() {
    // The caret between a and B stands after the underline control.
    let mut e = editor("a\\LB");
    caret_at(&mut e, 3);
    assert!(e.backspace(false, 0.0));
    assert_eq!(e.contents(), "\\LB", "the `a` goes, the control stays");
}

#[test]
fn moving_the_caret_and_extending_the_selection() {
    let mut e = editor("one two\\Pthree");
    e.move_caret(Move::DocStart, false);
    assert_eq!(e.selected_range(), 0..0);
    e.move_caret(Move::Right, false);
    e.move_caret(Move::Right, true);
    e.move_caret(Move::Right, true);
    assert_eq!(selected(&e), "ne");
    e.move_caret(Move::Left, true);
    assert_eq!(selected(&e), "n");
    e.move_caret(Move::Left, false);
    assert_eq!(
        e.selected_range(),
        1..1,
        "Left over a selection goes to its start"
    );
    e.move_caret(Move::WordRight, false);
    assert_eq!(e.selected_range(), 4..4);
    e.move_caret(Move::End, true);
    assert_eq!(selected(&e), "two");
    e.move_caret(Move::Home, false);
    assert_eq!(e.selected_range(), 0..0);
    e.move_caret(Move::DocEnd, true);
    assert_eq!(selected(&e), e.contents());
    e.move_caret(Move::Right, false);
    assert_eq!(
        e.selected_range().start,
        e.contents().len(),
        "Right over a selection goes to its end"
    );
    e.move_caret(Move::WordLeft, false);
    assert_eq!(e.selected_range(), 9..9);
}

#[test]
fn up_and_down_keep_their_column_across_a_short_line() {
    let mut e = editor("abcdef\\Pab\\Pabcdef");
    caret_at(&mut e, 5);
    e.move_caret(Move::Down, false);
    let short = e.selected_range().start;
    assert_eq!(short, 10, "the end of the short line");
    e.move_caret(Move::Down, false);
    assert_eq!(
        e.selected_range().start,
        17,
        "back to the same column on the long one"
    );
    e.move_caret(Move::Down, false);
    assert_eq!(e.selected_range().start, 18, "the last line's end");
    e.move_caret(Move::Up, false);
    e.move_caret(Move::Up, false);
    e.move_caret(Move::Up, false);
    assert_eq!(e.selected_range().start, 0, "the first line's start");
}

#[test]
fn copy_cut_and_paste_use_plain_text() {
    let mut e = editor("ab\\Pc\\H3;d");
    assert_eq!(e.copy_text(), None, "no selection, nothing to copy");
    e.select_all();
    assert_eq!(e.copy_text().as_deref(), Some("ab\ncd"));
    assert_eq!(e.cut_text(0.0).as_deref(), Some("ab\ncd"));
    assert_eq!(e.contents(), "");
    assert!(e.paste("x\r\ny\u{7}\tz{", 1.0));
    assert_eq!(e.contents(), "x\\Py\tz\\{");
    assert!(
        !e.paste("\u{1}\u{2}", 2.0),
        "only control characters is nothing"
    );
    assert!(!e.paste(&"a".repeat((1 << 20) + 1), 3.0));
    assert!(e.notice.is_some());
}

#[test]
fn an_edit_that_would_damage_hidden_formatting_is_refused_and_changes_nothing() {
    let mut e = editor("a\\pxqd;b");
    e.select_all();
    let steps = e.can_undo();
    assert!(!e.delete(false, 0.0));
    assert_eq!(e.contents(), "a\\pxqd;b");
    assert!(e.notice.as_deref().unwrap().contains("formatting"));
    assert_eq!(e.can_undo(), steps, "no step was recorded");
    assert!(e.has_selection(), "the selection is still there");
    // Typing at the end next to it works and clears the notice.
    let end = e.contents().len();
    caret_at(&mut e, end);
    assert!(e.insert("!", 1.0));
    assert!(e.notice.is_none());
}

#[test]
fn a_burst_of_typing_undoes_in_one_step_and_undo_restores_what_changed_state() {
    let mut e = editor("");
    for (i, c) in ["a", "b", "c"].iter().enumerate() {
        e.insert(c, i as f64 * 0.1);
    }
    e.insert("d", 5.0);
    assert_eq!(e.contents(), "abcd");
    assert!(e.undo());
    assert_eq!(e.contents(), "abc");
    assert!(e.undo());
    assert_eq!(e.contents(), "");
    assert!(!e.is_changed());
    assert!(!e.undo());
    assert!(e.redo() && e.redo());
    assert_eq!(e.contents(), "abcd");
    // Moving the caret ends the burst.
    let mut e = editor("");
    e.insert("a", 0.0);
    e.move_caret(Move::Left, false);
    e.insert("b", 0.1);
    assert!(e.undo());
    assert_eq!(e.contents(), "a");
}

#[test]
fn clicking_selects_words_and_lines() {
    let mut e = editor("one two\\Pthree");
    let layout = std::rc::Rc::clone(e.layout());
    let (x4, _, _) = layout.caret_rect(5).unwrap();
    let first = layout.lines[0].baseline;
    e.place_at(x4, first, false);
    assert_eq!(e.selected_range(), 5..5);
    e.place_at(0.0, first, true);
    assert_eq!(selected(&e), "one t");
    e.select_word_at(x4, first);
    assert_eq!(selected(&e), "two");
    e.select_line_at(x4, first);
    assert_eq!(selected(&e), "one two");
    let second = layout.lines[1].baseline;
    e.select_word_at(0.1, second);
    assert_eq!(selected(&e), "three");
    e.place_at(1e6, -1e6, false);
    assert_eq!(e.selected_range().start, e.contents().len());
}

#[test]
fn at_a_wrap_the_two_sides_are_told_apart() {
    let one = editor("aaaa").layout().actual_width;
    let mut e = editor_wrapped("aaaa aaaa", 1.3 * one);
    assert_eq!(e.layout().lines.len(), 2);
    // End on the first line stands at the end of it, not the start of the next.
    caret_at(&mut e, 0);
    e.move_caret(Move::End, false);
    assert!(e.active.upstream, "the end of the first line is upstream");
    assert_eq!(e.active.source, 5);
    let (x, _, _) = e.layout().caret_rect(e.caret()).unwrap();
    assert!(x > 0.0, "drawn at the end of the line");
    e.move_caret(Move::Down, false);
    assert!(!e.active.upstream);
    e.move_caret(Move::Home, false);
    assert_eq!(e.active.source, 5);
    assert_eq!(e.layout().carets[e.caret()].line, 1);
}

#[test]
fn undo_after_a_big_change_restores_the_layout_too() {
    let mut e = editor("a");
    e.insert("\nb\nc", 0.0);
    assert_eq!(e.layout().lines.len(), 3);
    e.undo();
    assert_eq!(e.layout().lines.len(), 1);
    assert_eq!(e.caret(), 1);
}

#[test]
fn select_all_then_delete_leaves_nothing_for_any_contents() {
    let cases = [
        "hello",
        r"hello ",
        r"a\Pb",
        r"a\P",
        r"{\Lab}",
        r"ab{\Lcd}",
        r"x \S1/2; y",
        r"\S1/2;",
        r"one two three four five six seven eight",
        r"ab\L",
    ];
    for width in [0.0, 12.0] {
        for contents in cases {
            let mut e = editor_wrapped(contents, width);
            e.select_all();
            assert!(e.delete(false, 0.0), "{contents:?} width {width}");
            assert_eq!(e.contents(), "", "{contents:?} width {width}");
        }
    }
}

#[test]
fn changing_the_style_relays_out_the_text_and_counts_as_a_change() {
    let mut e = editor("Hello");
    let before = e.layout().frame_width;
    let mut wide = TextStyle::new(TextStyleId(3), "Wide");
    wide.width_factor = 2.0;
    e.set_style(&wide);
    assert_eq!(e.style_id(), TextStyleId(3));
    assert!(e.is_changed());
    assert!(e.layout().frame_width > before * 1.5);
    assert_eq!(e.contents(), "Hello");
    e.set_style(&TextStyle::standard());
    assert!(!e.is_changed(), "back where it started");
}

fn named(value: &str) -> od_core::block::DataField {
    od_core::block::DataField::new(
        "AcVar",
        "x",
        Some(od_core::block::FieldValue::Text(value.into())),
    )
}

#[test]
fn a_field_goes_in_at_the_caret_as_a_marker_and_shows_its_value() {
    let mut e = editor("ab");
    caret_at(&mut e, 1);
    assert!(e.insert_field(named("XY"), 0.0));
    assert_eq!(e.contents(), format!("a{}b", od_core::mtext::marker(0)));
    assert_eq!(e.text().field_values(), ["XY"]);
    // The layout draws "aXYb": four glyphs, none skipped.
    assert_eq!(e.layout().glyphs.len(), 4);
    assert!(e.layout().diagnostics.is_empty());
    assert!(e.is_changed());
}

#[test]
fn inserting_a_field_replaces_the_selection_and_undoes_in_one_step() {
    let mut e = editor("abcd");
    e.anchor = Pos {
        source: 1,
        upstream: false,
    };
    e.active = Pos {
        source: 3,
        upstream: false,
    };
    assert!(e.insert_field(named("F"), 0.0));
    assert_eq!(e.contents(), format!("a{}d", od_core::mtext::marker(0)));
    assert!(e.undo());
    assert_eq!(e.contents(), "abcd");
    assert!(e.redo());
    assert_eq!(e.text().field_values(), ["F"], "redo finds its field again");
}

#[test]
fn a_second_field_gets_the_next_marker_and_pruning_drops_an_undone_one() {
    let mut e = editor("");
    assert!(e.insert_field(named("one"), 0.0));
    assert!(e.insert_field(named("two"), 1.0));
    assert!(e.contents().contains(&od_core::mtext::marker(1)));
    assert!(e.undo());
    let mut done = e.text().clone();
    assert_eq!(
        done.fields.len(),
        2,
        "the undone field lingers until the end"
    );
    done.prune_fields();
    assert_eq!(done.field_values(), ["one"]);
    assert_eq!(done.contents, od_core::mtext::marker(0));
}
