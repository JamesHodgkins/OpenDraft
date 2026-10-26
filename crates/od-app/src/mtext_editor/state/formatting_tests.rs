//! Tests of character formatting in the editor.

use od_core::text::{CharEdit, Mixed};

use super::tests::{caret_at, editor, selected};
use super::*;

fn underline() -> CharEdit {
    CharEdit {
        underline: Some(true),
        ..CharEdit::default()
    }
}

#[test]
fn formatting_a_selection_keeps_it_selected_and_is_one_undoable_step() {
    let mut e = editor("hello");
    caret_at(&mut e, 1);
    e.move_caret(Move::Right, true);
    e.move_caret(Move::Right, true);
    e.move_caret(Move::Right, true);
    assert_eq!(selected(&e), "ell");
    assert!(e.set_format(underline(), 0.0));
    assert_eq!(e.contents(), "h{\\Lell}o");
    assert_eq!(selected(&e), "ell", "the same text stays selected");
    assert_eq!(e.format().underline, Mixed::Uniform(true));
    // Toggling it back reuses the group.
    let off = CharEdit {
        underline: Some(false),
        ..CharEdit::default()
    };
    assert!(e.set_format(off, 1.0));
    assert_eq!(e.contents(), "h{\\lell}o");
    assert_eq!(e.format().underline, Mixed::Uniform(false));
    assert!(e.undo() && e.undo());
    assert_eq!(e.contents(), "hello");
    assert_eq!(selected(&e), "ell");
    assert!(!e.is_changed());
}

#[test]
fn a_selection_across_different_formats_shows_mixed() {
    let mut e = editor("ab\\Lcd\\l");
    e.select_all();
    assert_eq!(e.format().underline, Mixed::Mixed);
    assert!(e.set_format(underline(), 0.0));
    assert_eq!(e.format().underline, Mixed::Uniform(true));
}

#[test]
fn a_format_chosen_at_a_caret_is_taken_by_what_is_typed_next() {
    let mut e = editor("ab");
    assert!(e.set_format(underline(), 0.0));
    assert!(e.has_pending_format());
    assert_eq!(e.contents(), "ab", "nothing changes yet");
    assert_eq!(
        e.format().underline,
        Mixed::Uniform(true),
        "the toolbar shows it"
    );
    assert!(e.insert("c", 0.1));
    assert!(e.insert("d", 0.2));
    assert_eq!(e.contents(), "ab{\\Lcd}", "typing went into one group");
    assert!(
        e.has_pending_format(),
        "the format stays until the caret moves"
    );
    assert!(e.insert("{", 0.3));
    assert_eq!(e.contents(), "ab{\\Lcd\\{}");
    // It was one burst, so one undo.
    assert!(e.undo());
    assert_eq!(e.contents(), "ab");
    assert!(!e.has_pending_format());
    // Backspace takes the last letter typed inside the group.
    e.set_format(underline(), 1.0);
    e.insert("xy", 1.1);
    assert!(e.backspace(false, 5.0));
    assert_eq!(e.contents(), "ab{\\Lx}");
}

#[test]
fn moving_the_caret_drops_a_pending_format() {
    let mut e = editor("ab");
    e.set_format(underline(), 0.0);
    e.move_caret(Move::Left, false);
    assert!(!e.has_pending_format());
    e.insert("x", 1.0);
    assert_eq!(e.contents(), "axb");
    e.set_format(underline(), 2.0);
    e.select_all();
    assert!(!e.has_pending_format());
}

#[test]
fn pending_changes_add_up_and_typing_over_a_selection_takes_them() {
    let mut e = editor("one two");
    e.set_format(underline(), 0.0);
    e.set_format(
        CharEdit {
            height: Some(4.0),
            ..CharEdit::default()
        },
        0.1,
    );
    assert_eq!(e.format().height, Mixed::Uniform(4.0));
    e.insert("!", 0.2);
    assert_eq!(e.contents(), "one two{\\L\\H4;!}");
    // Over a selection: the selection goes and the new text is formatted.
    let mut e = editor("one two");
    e.place_at(1e6, 0.0, false);
    e.move_caret(Move::WordLeft, true);
    assert_eq!(selected(&e), "two");
    e.set_format(underline(), 0.0);
    assert_eq!(
        e.contents(),
        "one {\\Ltwo}",
        "a selection is formatted at once"
    );
}

#[test]
fn a_refused_format_leaves_the_text_and_says_why() {
    let mut e = editor("a\\pxi;b");
    e.select_all();
    assert!(!e.set_format(underline(), 0.0));
    assert_eq!(e.contents(), "a\\pxi;b");
    assert!(e.notice.is_some());
    assert!(!e.can_undo());
    assert!(!e.set_format(CharEdit::default(), 0.0), "nothing to apply");
}

pub(super) fn bold_edit() -> CharEdit {
    CharEdit {
        bold: Some(true),
        ..CharEdit::default()
    }
}

#[test]
fn bold_on_a_selection_and_as_a_pending_format() {
    use od_core::text::FontFace;
    let mut e = editor("hello");
    caret_at(&mut e, 1);
    for _ in 0..3 {
        e.move_caret(Move::Right, true);
    }
    assert!(e.set_format(bold_edit(), 0.0));
    assert_eq!(e.contents(), "h{\\fDejaVu Sans|b1|i0;ell}o");
    assert_eq!(selected(&e), "ell");
    assert_eq!(e.format().bold, Mixed::Uniform(true));
    let faces: Vec<_> = e.layout().glyphs.iter().map(|g| g.face).collect();
    assert_eq!(faces[1], FontFace::Bold);
    assert_eq!(faces[0], FontFace::Regular);
    assert!(e.undo());
    assert_eq!(e.contents(), "hello");

    let mut e = editor("ab");
    e.set_format(bold_edit(), 1.0);
    e.insert("cd", 1.1);
    assert_eq!(e.contents(), "ab{\\fDejaVu Sans|b1|i0;cd}");
    assert_eq!(e.layout().glyphs[2].face, FontFace::Bold);
}

#[test]
fn control_b_and_control_i_toggle_bold_and_italic() {
    use crate::mtext_editor::input::handle_events;
    use eframe::egui::{Event, Key, Modifiers};
    let key = |key| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    let mut e = editor("word");
    handle_events(&mut e, &[key(Key::A), key(Key::B)], 0.0);
    assert_eq!(e.format().bold, Mixed::Uniform(true));
    handle_events(&mut e, &[key(Key::I)], 1.0);
    assert_eq!(e.format().italic, Mixed::Uniform(true));
    assert_eq!(
        e.format().bold,
        Mixed::Uniform(true),
        "italic keeps the bold"
    );
    handle_events(&mut e, &[key(Key::B)], 2.0);
    assert_eq!(e.format().bold, Mixed::Uniform(false));
    assert_eq!(e.format().italic, Mixed::Uniform(true));
}

#[test]
fn selected_text_is_stacked_and_unstacked_in_one_undoable_step_each() {
    let mut e = editor("x 1/2 y");
    // Select the `1/2`.
    caret_at(&mut e, 2);
    for _ in 0..3 {
        e.move_caret(Move::Right, true);
    }
    assert_eq!(selected(&e), "1/2");
    assert!(e.toggle_stack(0.0));
    assert_eq!(e.contents(), "x \\S1/2; y");
    assert_eq!(selected(&e), "\\S1/2;", "the stack stays selected");
    assert_eq!(
        e.layout().carets.len(),
        6,
        "the stack is one step among the letters and spaces"
    );
    assert!(e.toggle_stack(1.0));
    assert_eq!(e.contents(), "x 1/2 y");
    assert_eq!(selected(&e), "1/2");
    assert!(e.undo() && e.undo());
    assert_eq!(e.contents(), "x 1/2 y");
    assert!(!e.is_changed());
}

#[test]
fn stacking_needs_a_selection_with_a_divider_and_says_why_not() {
    let mut e = editor("abc");
    assert!(!e.toggle_stack(0.0), "nothing selected");
    assert!(e.notice.is_some());
    e.select_all();
    assert!(!e.toggle_stack(1.0), "no divider");
    assert!(e.notice.as_deref().unwrap().contains("1/2"));
    assert_eq!(e.contents(), "abc");
    assert!(!e.can_undo());
}

#[test]
fn deleting_a_stack_takes_it_whole() {
    let mut e = editor("x\\S1/2;y");
    caret_at(&mut e, 7);
    assert!(e.backspace(false, 0.0));
    assert_eq!(e.contents(), "xy");
}

#[test]
fn tracking_applies_to_every_letter_of_the_selection() {
    let mut e = editor("hello");
    e.select_all();
    let edit = CharEdit {
        tracking: Some(2.0),
        ..CharEdit::default()
    };
    assert!(e.set_format(edit, 0.0));
    assert_eq!(e.contents(), r"{\T2;hello}");
    assert_eq!(selected(&e), "hello");
    // A second change keeps working on all of it.
    let again = CharEdit {
        tracking: Some(3.0),
        ..CharEdit::default()
    };
    assert!(e.set_format(again, 1.0));
    assert_eq!(e.format().tracking, Mixed::Uniform(3.0));
    assert_eq!(selected(&e), "hello");
}

#[test]
fn the_highlight_keeps_the_last_letter_while_the_format_is_dragged() {
    let mut e = editor("HELLO WORLD");
    e.select_all();
    for (i, t) in [1.2, 1.5, 2.0, 3.0].into_iter().enumerate() {
        let edit = CharEdit {
            tracking: Some(t),
            ..CharEdit::default()
        };
        assert!(e.adjust_format(edit, 0.1 * i as f64));
        let (from, to) = e.selection();
        let rects = e.layout().selection_rects(from, to);
        let right = rects.iter().map(|r| r.x1).fold(0.0, f64::max);
        let last = e.layout().glyphs.last().unwrap();
        assert!(
            right >= last.x,
            "tracking {t}: the highlight ends at {right}, the last letter starts at {}",
            last.x
        );
        assert_eq!(to, e.layout().carets.len() - 1, "tracking {t}");
    }
}

/// Select the bytes `from..to` of the contents.
fn select_bytes(e: &mut MTextEditor, from: usize, to: usize) {
    let at = |source| Pos {
        source,
        upstream: false,
    };
    e.anchor = at(from);
    e.active = at(to);
}

#[test]
fn spacing_and_width_on_a_selection_that_cuts_a_group_are_not_refused() {
    let mut e = editor("hello world");
    select_bytes(&mut e, 1, 5);
    let tracking = CharEdit {
        tracking: Some(2.0),
        ..CharEdit::default()
    };
    assert!(e.set_format(tracking, 0.0));
    assert_eq!(e.contents(), "h{\\T2;ello} world");
    // From inside the spaced text to past its end: half a group.
    select_bytes(&mut e, 8, 13);
    let width = |v| CharEdit {
        width: Some(v),
        ..CharEdit::default()
    };
    assert!(
        e.adjust_format(width(2.0), 1.0),
        "{:?}",
        e.notice.as_deref()
    );
    assert_eq!(e.notice.as_deref(), None);
    assert_eq!(e.contents(), "h{\\T2;el{\\W2;lo}}{\\W2; w}orld");
    // Dragging the value on goes on landing without nesting deeper.
    let braces = e.contents().matches('{').count();
    for step in 3..8 {
        assert!(e.adjust_format(width(step as f64), 1.1));
        assert_eq!(e.contents().matches('{').count(), braces);
    }
    assert!(e.undo());
    assert_eq!(
        e.contents(),
        "h{\\T2;ello} world",
        "one undo step for the drag"
    );
    // Typing over a selection that cuts a group keeps the groups balanced.
    select_bytes(&mut e, 8, 13);
    assert!(e.insert("X", 2.0));
    let (open, close) = (
        e.contents().matches('{').count(),
        e.contents().matches('}').count(),
    );
    assert_eq!(open, close);
}
