//! The steps of the editing-session stress test: the model, the operations
//! whose result it can state exactly, and the ones it cannot. See
//! `stress_model.rs`.

use eframe::egui::{Event, Key, Modifiers};
use od_core::text::plain_text_of;

use super::stress_ops::Rng;
use super::MTextEditor;
use crate::mtext_editor::input::handle_events;

/// What the editor should be showing: the letters, the selection as letter
/// positions, and the clipboard.
pub(super) struct Model {
    pub(super) text: Vec<char>,
    pub(super) anchor: usize,
    pub(super) active: usize,
    pub(super) clipboard: String,
}

impl Model {
    pub(super) fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.active), self.anchor.max(self.active))
    }

    pub(super) fn has_selection(&self) -> bool {
        self.anchor != self.active
    }

    pub(super) fn put(&mut self, at: usize) {
        self.active = at;
        self.anchor = at;
    }

    pub(super) fn replace_selection(&mut self, with: &str) {
        let (a, b) = self.range();
        let new: Vec<char> = with.chars().collect();
        let end = a + new.len();
        self.text.splice(a..b, new);
        self.put(end);
    }

    pub(super) fn line_start(&self, at: usize) -> usize {
        self.text[..at]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1)
    }

    pub(super) fn line_end(&self, at: usize) -> usize {
        self.text[at..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(self.text.len(), |i| at + i)
    }

    pub(super) fn selected(&self) -> String {
        let (a, b) = self.range();
        self.text[a..b].iter().collect()
    }
}

pub(super) fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

pub(super) const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

/// One step of a session: the events to send and what the model does.
pub(super) fn step(rng: &mut Rng, m: &mut Model, wrapped: bool) -> (Vec<Event>, String) {
    let letters = [
        'a', 'b', 'c', 'd', 'x', 'y', 'z', ' ', ' ', '{', '}', '\\', ';', 'é', '漢',
    ];
    let extend = rng.chance(35);
    let mods = if extend { SHIFT } else { Modifiers::NONE };
    let n = m.text.len();
    // Home and End go to the start and end of the line on screen, which is
    // the paragraph only when nothing wraps; a wrapped text steps instead.
    let roll = rng.below(100);
    let roll = if wrapped && (70..=79).contains(&roll) {
        roll - 25
    } else {
        roll
    };
    match roll {
        0..=24 => {
            let word: String = (0..1 + rng.below(5)).map(|_| *rng.pick(&letters)).collect();
            m.replace_selection(&word);
            (vec![Event::Text(word.clone())], format!("type {word:?}"))
        }
        25..=29 => {
            m.replace_selection("\t");
            (vec![key(Key::Tab, Modifiers::NONE)], "Tab".into())
        }
        30..=34 => {
            m.replace_selection("\n");
            (vec![key(Key::Enter, Modifiers::NONE)], "Enter".into())
        }
        35..=40 => {
            if m.has_selection() {
                m.replace_selection("");
            } else if m.active > 0 {
                m.text.remove(m.active - 1);
                m.put(m.active - 1);
            }
            (
                vec![key(Key::Backspace, Modifiers::NONE)],
                "Backspace".into(),
            )
        }
        41..=45 => {
            if m.has_selection() {
                m.replace_selection("");
            } else if m.active < n {
                m.text.remove(m.active);
            }
            (vec![key(Key::Delete, Modifiers::NONE)], "Delete".into())
        }
        46..=57 => {
            // With a selection and no Shift, the caret goes to its left edge.
            let to = if m.has_selection() && !extend {
                m.range().0
            } else {
                m.active.saturating_sub(1)
            };
            m.active = to;
            if !extend {
                m.anchor = to;
            }
            (vec![key(Key::ArrowLeft, mods)], format!("Left {extend}"))
        }
        58..=69 => {
            let to = if m.has_selection() && !extend {
                m.range().1
            } else {
                (m.active + 1).min(n)
            };
            m.active = to;
            if !extend {
                m.anchor = to;
            }
            (vec![key(Key::ArrowRight, mods)], format!("Right {extend}"))
        }
        70..=74 => {
            m.active = m.line_start(m.active);
            if !extend {
                m.anchor = m.active;
            }
            (vec![key(Key::Home, mods)], format!("Home {extend}"))
        }
        75..=79 => {
            m.active = m.line_end(m.active);
            if !extend {
                m.anchor = m.active;
            }
            (vec![key(Key::End, mods)], format!("End {extend}"))
        }
        80..=81 => {
            m.active = 0;
            if !extend {
                m.anchor = 0;
            }
            let ctrl = Modifiers {
                shift: extend,
                ..Modifiers::COMMAND
            };
            (vec![key(Key::Home, ctrl)], format!("Ctrl+Home {extend}"))
        }
        82..=83 => {
            m.active = n;
            if !extend {
                m.anchor = n;
            }
            let ctrl = Modifiers {
                shift: extend,
                ..Modifiers::COMMAND
            };
            (vec![key(Key::End, ctrl)], format!("Ctrl+End {extend}"))
        }
        84 => {
            (m.anchor, m.active) = (0, n);
            (vec![key(Key::A, Modifiers::COMMAND)], "Ctrl+A".into())
        }
        85..=88 => {
            if m.has_selection() {
                m.clipboard = m.selected();
            }
            (vec![Event::Copy], "Copy".into())
        }
        89..=93 => {
            if m.has_selection() {
                m.clipboard = m.selected();
                m.replace_selection("");
            }
            (vec![Event::Cut], "Cut".into())
        }
        _ => {
            let clip = m.clipboard.clone();
            if !clip.is_empty() {
                m.replace_selection(&clip);
            }
            (vec![Event::Paste(clip)], "Paste".into())
        }
    }
}

/// The letters before caret `index` of the layout.
pub(super) fn letters_before(e: &MTextEditor, index: usize) -> usize {
    let byte = e.layout().carets.get(index).map_or(0, |c| c.source);
    plain_text_of(e.contents(), 0..byte).chars().count()
}

/// What the model cannot say exactly (where a word or a screen line ends, or
/// where a click lands): the editor decides, and these are checked instead.
pub(super) enum Free {
    /// The caret moved or a selection was made: the text must not change.
    Moved,
    /// A word was deleted: backwards or forwards from the caret.
    Deleted { backwards: bool },
}

pub(super) fn free_step(rng: &mut Rng, e: &mut MTextEditor, now: f64) -> (Free, String) {
    let shift = rng.chance(35);
    let (x, y) = (rng.unit() * 40.0 - 3.0, -rng.unit() * 20.0 + 2.0);
    let mods = |command: bool| Modifiers {
        shift,
        command,
        ..Modifiers::NONE
    };
    let keys = |events: Vec<Event>, e: &mut MTextEditor| {
        handle_events(e, &events, now);
    };
    match rng.below(10) {
        0 => {
            keys(vec![key(Key::ArrowLeft, mods(true))], e);
            (Free::Moved, format!("Ctrl+Left {shift}"))
        }
        1 => {
            keys(vec![key(Key::ArrowRight, mods(true))], e);
            (Free::Moved, format!("Ctrl+Right {shift}"))
        }
        2 => {
            keys(vec![key(Key::ArrowUp, mods(false))], e);
            (Free::Moved, format!("Up {shift}"))
        }
        3 => {
            keys(vec![key(Key::ArrowDown, mods(false))], e);
            (Free::Moved, format!("Down {shift}"))
        }
        4 => {
            keys(vec![key(Key::Backspace, Modifiers::COMMAND)], e);
            (Free::Deleted { backwards: true }, "Ctrl+Backspace".into())
        }
        5 => {
            keys(vec![key(Key::Delete, Modifiers::COMMAND)], e);
            (Free::Deleted { backwards: false }, "Ctrl+Delete".into())
        }
        6 => {
            e.place_at(x, y, shift);
            (Free::Moved, format!("click at {x:.1},{y:.1} {shift}"))
        }
        7 => {
            e.select_word_at(x, y);
            (Free::Moved, format!("double-click at {x:.1},{y:.1}"))
        }
        8 => {
            e.select_line_at(x, y);
            (Free::Moved, format!("triple-click at {x:.1},{y:.1}"))
        }
        _ => {
            e.place_at(x, y, false);
            e.place_at(x + 8.0, y, true);
            (Free::Moved, format!("drag from {x:.1},{y:.1}"))
        }
    }
}
