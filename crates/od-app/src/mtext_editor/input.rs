//! Keys, typed text, the clipboard and input-method events, turned into
//! editor operations. Nothing here touches egui's context, so it is tested
//! with made-up events.

use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use od_core::text::CharEdit;

use super::state::{MTextEditor, Move};

/// How the editor ended, when it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accept,
    Cancel,
}

/// What a frame's events did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Handled {
    pub outcome: Option<Outcome>,
    /// Text for the system clipboard (Copy or Cut).
    pub copied: Option<String>,
    /// Whether the draft or the caret moved, so the caret shows solid.
    pub touched: bool,
}

/// Most characters a frame will take in, so a flood of events (a huge paste,
/// a stuck key) cannot hold the window up.
const MAX_SCALARS_PER_FRAME: usize = 4096;

fn movement(key: Key, m: Modifiers) -> Option<Move> {
    Some(match (key, m.command) {
        (Key::ArrowLeft, false) => Move::Left,
        (Key::ArrowRight, false) => Move::Right,
        (Key::ArrowLeft, true) => Move::WordLeft,
        (Key::ArrowRight, true) => Move::WordRight,
        (Key::ArrowUp, _) => Move::Up,
        (Key::ArrowDown, _) => Move::Down,
        (Key::Home, false) => Move::Home,
        (Key::End, false) => Move::End,
        (Key::Home, true) => Move::DocStart,
        (Key::End, true) => Move::DocEnd,
        _ => return None,
    })
}

/// Typed text without control characters (a stray Enter or Escape that
/// arrives as text is a key, not a letter).
fn typed(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub fn handle_events(editor: &mut MTextEditor, events: &[Event], now: f64) -> Handled {
    let mut done = Handled::default();
    let mut budget = MAX_SCALARS_PER_FRAME;
    let mut take = |text: &str| -> Option<String> {
        let count = text.chars().count();
        if count > budget {
            return None;
        }
        budget -= count;
        Some(text.to_owned())
    };
    // Text typed in one frame goes in as one edit, not one layout per letter.
    let mut pending = String::new();
    for event in events {
        if done.outcome.is_some() {
            break;
        }
        if let Event::Text(text) = event {
            if let Some(text) = take(&typed(text)) {
                pending.push_str(&text);
            }
            continue;
        }
        if !pending.is_empty() {
            done.touched |= editor.insert(&std::mem::take(&mut pending), now);
        }
        match event {
            Event::Ime(ImeEvent::Commit(text)) => {
                editor.composing = None;
                if let Some(text) = take(&typed(text)) {
                    done.touched |= editor.insert(&text, now);
                }
            }
            Event::Ime(ImeEvent::Preedit(text)) => {
                editor.composing = (!text.is_empty()).then(|| text.clone());
            }
            Event::Ime(_) => editor.composing = None,
            Event::Paste(text) => {
                if let Some(text) = take(text) {
                    done.touched |= editor.paste(&text, now);
                } else {
                    editor.notice = Some("the pasted text is too long".to_owned());
                }
            }
            Event::Copy => done.copied = editor.copy_text().or(done.copied.take()),
            Event::Cut => {
                if let Some(text) = editor.cut_text(now) {
                    done.copied = Some(text);
                    done.touched = true;
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => key_pressed(editor, *key, *modifiers, now, &mut done),
            _ => {}
        }
    }
    if done.outcome.is_none() && !pending.is_empty() {
        done.touched |= editor.insert(&pending, now);
    }
    done
}

fn key_pressed(editor: &mut MTextEditor, key: Key, m: Modifiers, now: f64, done: &mut Handled) {
    if let Some(to) = movement(key, m) {
        editor.move_caret(to, m.shift);
        done.touched = true;
        return;
    }
    match key {
        Key::Escape => done.outcome = Some(Outcome::Cancel),
        Key::Enter if m.command => done.outcome = Some(Outcome::Accept),
        Key::Enter => done.touched |= editor.insert("\n", now),
        Key::Tab if !m.command => done.touched |= editor.insert("\t", now),
        Key::Backspace => done.touched |= editor.backspace(m.command, now),
        Key::Delete => done.touched |= editor.delete(m.command, now),
        Key::U if m.command => {
            let on = editor.format().underline.uniform().unwrap_or(false);
            let edit = CharEdit {
                underline: Some(!on),
                ..CharEdit::default()
            };
            done.touched |= editor.set_format(edit, now);
        }
        Key::B if m.command => {
            let on = editor.format().bold.uniform().unwrap_or(false);
            let edit = CharEdit {
                bold: Some(!on),
                ..CharEdit::default()
            };
            done.touched |= editor.set_format(edit, now);
        }
        Key::I if m.command => {
            let on = editor.format().italic.uniform().unwrap_or(false);
            let edit = CharEdit {
                italic: Some(!on),
                ..CharEdit::default()
            };
            done.touched |= editor.set_format(edit, now);
        }
        Key::A if m.command => {
            editor.select_all();
            done.touched = true;
        }
        Key::Z if m.command && m.shift => done.touched |= editor.redo(),
        Key::Z if m.command => done.touched |= editor.undo(),
        Key::Y if m.command => done.touched |= editor.redo(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use od_core::{MText, Point, TextStyle, TextStyleId};

    use super::*;

    fn editor(contents: &str) -> MTextEditor {
        let text = MText::new(Point::new(0.0, 0.0), 2.0, contents, TextStyleId::STANDARD);
        MTextEditor::new(text, TextStyle::standard())
    }

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn plain(k: Key) -> Event {
        key(k, Modifiers::NONE)
    }

    fn ctrl(k: Key) -> Event {
        key(k, Modifiers::COMMAND)
    }

    #[test]
    fn typed_text_and_enter_and_tab_go_into_the_draft() {
        let mut e = editor("");
        let done = handle_events(
            &mut e,
            &[
                Event::Text("ab".into()),
                plain(Key::Enter),
                Event::Text("c\u{7}d".into()),
                plain(Key::Tab),
            ],
            0.0,
        );
        assert_eq!(e.contents(), "ab\\Pcd\t");
        assert!(done.touched && done.outcome.is_none());
        // A control character arriving as text is not a letter.
        handle_events(&mut e, &[Event::Text("\r".into())], 1.0);
        assert_eq!(e.contents(), "ab\\Pcd\t");
    }

    #[test]
    fn keys_move_edit_select_and_undo() {
        let mut e = editor("one two");
        handle_events(
            &mut e,
            &[
                plain(Key::Backspace),
                key(Key::ArrowLeft, Modifiers::SHIFT),
                key(Key::ArrowLeft, Modifiers::SHIFT),
            ],
            0.0,
        );
        assert_eq!(e.contents(), "one tw");
        assert_eq!(&e.contents()[e.selected_range()], "tw");
        handle_events(&mut e, &[plain(Key::Delete)], 1.0);
        assert_eq!(e.contents(), "one ");
        handle_events(&mut e, &[ctrl(Key::Z)], 2.0);
        assert_eq!(e.contents(), "one tw");
        handle_events(
            &mut e,
            &[key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            3.0,
        );
        assert_eq!(e.contents(), "one ");
        handle_events(&mut e, &[ctrl(Key::Z), ctrl(Key::Y)], 4.0);
        assert_eq!(e.contents(), "one ");
        handle_events(&mut e, &[ctrl(Key::A), plain(Key::Backspace)], 5.0);
        assert_eq!(e.contents(), "");
        let mut e = editor("one two");
        handle_events(&mut e, &[ctrl(Key::Backspace)], 0.0);
        assert_eq!(e.contents(), "one ", "Ctrl+Backspace takes a word");
        handle_events(&mut e, &[ctrl(Key::Home), ctrl(Key::Delete)], 1.0);
        assert_eq!(e.contents(), "");
    }

    #[test]
    fn arrows_with_control_jump_words_and_home_and_end_go_to_the_edges() {
        let mut e = editor("one two\\Pthree");
        handle_events(&mut e, &[ctrl(Key::Home)], 0.0);
        assert_eq!(e.selected_range(), 0..0);
        handle_events(&mut e, &[ctrl(Key::ArrowRight)], 0.0);
        assert_eq!(e.selected_range(), 4..4);
        handle_events(&mut e, &[plain(Key::End)], 0.0);
        assert_eq!(e.selected_range(), 7..7);
        handle_events(&mut e, &[plain(Key::ArrowDown), plain(Key::Home)], 0.0);
        assert_eq!(e.selected_range(), 9..9);
        handle_events(&mut e, &[ctrl(Key::End)], 0.0);
        assert_eq!(e.selected_range(), 14..14);
    }

    #[test]
    fn escape_and_control_enter_end_the_editor_and_stop_further_events() {
        let mut e = editor("");
        let done = handle_events(
            &mut e,
            &[
                Event::Text("a".into()),
                ctrl(Key::Enter),
                Event::Text("b".into()),
            ],
            0.0,
        );
        assert_eq!(done.outcome, Some(Outcome::Accept));
        assert_eq!(e.contents(), "a", "nothing after it is taken");
        let done = handle_events(&mut e, &[plain(Key::Escape)], 1.0);
        assert_eq!(done.outcome, Some(Outcome::Cancel));
        // Releasing a key does nothing.
        let release = Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(handle_events(&mut e, &[release], 2.0), Handled::default());
    }

    #[test]
    fn the_clipboard_events_copy_cut_and_paste_plain_text() {
        let mut e = editor("ab\\Pcd");
        assert_eq!(handle_events(&mut e, &[Event::Copy], 0.0).copied, None);
        handle_events(&mut e, &[ctrl(Key::A)], 0.0);
        let done = handle_events(&mut e, &[Event::Copy], 0.0);
        assert_eq!(done.copied.as_deref(), Some("ab\ncd"));
        assert_eq!(e.contents(), "ab\\Pcd", "copy changes nothing");
        let done = handle_events(&mut e, &[Event::Cut], 1.0);
        assert_eq!(done.copied.as_deref(), Some("ab\ncd"));
        assert_eq!(e.contents(), "");
        handle_events(&mut e, &[Event::Paste("x\ny".into())], 2.0);
        assert_eq!(e.contents(), "x\\Py");
    }

    #[test]
    fn input_method_composition_is_shown_and_only_the_commit_is_typed() {
        let mut e = editor("");
        handle_events(&mut e, &[Event::Ime(ImeEvent::Enabled)], 0.0);
        handle_events(&mut e, &[Event::Ime(ImeEvent::Preedit("にほ".into()))], 0.0);
        assert_eq!(e.composing.as_deref(), Some("にほ"));
        assert_eq!(e.contents(), "", "not in the draft yet");
        assert!(!e.can_undo());
        handle_events(&mut e, &[Event::Ime(ImeEvent::Commit("日本".into()))], 1.0);
        assert_eq!(e.contents(), "日本");
        assert_eq!(e.composing, None);
        handle_events(
            &mut e,
            &[
                Event::Ime(ImeEvent::Preedit("x".into())),
                Event::Ime(ImeEvent::Disabled),
            ],
            2.0,
        );
        assert_eq!(e.composing, None);
    }

    #[test]
    fn a_flood_of_input_is_cut_off_at_the_frame_budget() {
        let mut e = editor("");
        let big = "a".repeat(5000);
        handle_events(&mut e, &[Event::Text(big.clone())], 0.0);
        assert_eq!(
            e.contents(),
            "",
            "one event over the budget is dropped whole"
        );
        handle_events(&mut e, &[Event::Paste(big)], 1.0);
        assert_eq!(e.contents(), "");
        assert!(e.notice.is_some());
        let many: Vec<Event> = (0..5000).map(|_| Event::Text("b".into())).collect();
        handle_events(&mut e, &many, 2.0);
        assert_eq!(e.contents().len(), 4096);
    }

    #[test]
    fn a_refused_edit_leaves_a_notice_and_the_text_alone() {
        let mut e = editor("a\\pxi;b");
        handle_events(&mut e, &[ctrl(Key::A), plain(Key::Backspace)], 0.0);
        assert_eq!(e.contents(), "a\\pxi;b");
        assert!(e.notice.is_some());
    }
}
