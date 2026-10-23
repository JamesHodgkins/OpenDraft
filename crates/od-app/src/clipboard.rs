//! Cut, Copy, Copy with Base Point, Paste, Paste as Block and Paste to
//! Original Coordinates.
//!
//! What is copied is a small drawing (`Document::extract`): the objects and
//! the tables they might need. Pasting brings it into the active drawing with
//! `Document::absorb`, which matches layers, styles, linetypes and blocks by
//! name and adds the ones that are missing, so the same path serves the
//! same drawing, another open drawing and another OpenDraft window. The
//! drawing is also put on the system clipboard as text, which is how another
//! window gets it; other programs just see the text.

use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui;
use od_commands::{Cancelled, Command, Editor, PasteDrawing, PointOptions, RemoveEntities};
use od_core::{Document, Entity, EntityId, Placement, Point};
use od_files::{FileFormat, NativeFormat};

use crate::state::AppState;
use crate::text::plural;

/// The first word of the text put on the system clipboard.
const MARK: &str = "opendraft-clipboard";

/// What Copy and Cut last put aside.
#[derive(Clone)]
pub struct Clipboard {
    drawing: Document,
    /// The point that lands on the insertion point when pasting.
    base: Point,
}

impl Clipboard {
    /// The objects `ids` of `document`; with no `base` it is the lower left
    /// corner of what is copied. `None` when none of them exist.
    fn of(document: &Document, ids: &[EntityId], base: Option<Point>) -> Option<Clipboard> {
        let drawing = document.extract(ids);
        let corner = drawing
            .objects()
            .iter()
            .filter_map(|o| {
                // A construction line or ray has no corner: its base point.
                let b = o.entity.bounds();
                match o.entity.infinite_line() {
                    Some((origin, ..)) => Some(origin),
                    None => b.is_finite().then_some(b.min),
                }
            })
            .reduce(|a, b| Point::new(a.x.min(b.x), a.y.min(b.y)))?;
        Some(Clipboard {
            drawing,
            base: base.unwrap_or(corner),
        })
    }

    fn count(&self) -> usize {
        self.drawing.objects().len()
    }

    /// The text for the system clipboard.
    fn to_text(&self) -> Option<String> {
        let encoded = NativeFormat.write(&self.drawing).ok()?;
        let json = String::from_utf8(encoded.bytes).ok()?;
        Some(format!(
            "{MARK} {:?} {:?}\n{json}",
            self.base.x, self.base.y
        ))
    }

    fn from_text(text: &str) -> Option<Clipboard> {
        let (head, json) = text.split_once('\n')?;
        let mut words = head.split(' ');
        if words.next()? != MARK {
            return None;
        }
        let base = Point::new(words.next()?.parse().ok()?, words.next()?.parse().ok()?);
        let drawing = NativeFormat.read(json.as_bytes()).ok()?.document;
        Some(Clipboard { drawing, base })
    }

    fn shifted(&self, to: Point) -> Vec<Entity> {
        let (dx, dy) = (to.x - self.base.x, to.y - self.base.y);
        self.drawing
            .objects()
            .iter()
            .map(|o| o.entity.translated(dx, dy))
            .collect()
    }
}

#[derive(Default)]
struct Inner {
    clipboard: Option<Clipboard>,
    /// The text last put on the system clipboard. When the system clipboard
    /// holds something else, something else was copied since.
    written: String,
}

/// The clipboard, shared with the command thread that COPYBASE runs on.
#[derive(Clone, Default)]
pub struct ClipboardSlot(Arc<Mutex<Inner>>);

impl ClipboardSlot {
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, clipboard: Clipboard) {
        let text = clipboard.to_text().unwrap_or_default();
        if !text.is_empty() {
            system::write(&text);
        }
        let mut inner = self.inner();
        inner.written = text;
        inner.clipboard = Some(clipboard);
    }

    /// What a paste would bring: a drawing copied in another window (or
    /// since this one copied), else the last thing copied here.
    fn current(&self) -> Option<Clipboard> {
        let seen = system::read();
        let mut inner = self.inner();
        if let Some(text) = seen.filter(|t| *t != inner.written) {
            if let Some(foreign) = Clipboard::from_text(&text) {
                inner.written = text;
                inner.clipboard = Some(foreign);
            }
        }
        inner.clipboard.clone()
    }
}

/// The system clipboard's text. Tests use a stand-in so they do not touch it.
#[cfg(not(test))]
mod system {
    pub fn read() -> Option<String> {
        arboard::Clipboard::new().ok()?.get_text().ok()
    }

    pub fn write(text: &str) {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            // Another program holding the clipboard open is not worth a message.
            let _ = clipboard.set_text(text.to_owned());
        }
    }
}

#[cfg(test)]
mod system {
    use std::cell::RefCell;

    thread_local!(pub static TEXT: RefCell<Option<String>> = const { RefCell::new(None) });

    pub fn read() -> Option<String> {
        TEXT.with(|t| t.borrow().clone())
    }

    pub fn write(text: &str) {
        TEXT.with(|t| *t.borrow_mut() = Some(text.to_owned()));
    }
}

/// How a paste places what it brings.
#[derive(Clone, Copy, PartialEq)]
enum Paste {
    /// At a point asked for.
    At,
    /// As one block at a point asked for.
    Block,
    /// Where it was copied from.
    Original,
}

impl AppState {
    /// Copy `ids` of the active drawing to the clipboard.
    pub(crate) fn copy_to_clipboard(&mut self, ids: &[EntityId], base: Option<Point>) {
        let document = self.docs[self.active].session.document();
        match Clipboard::of(document, ids, base) {
            Some(clip) => {
                self.message = format!("Copied {}", plural(clip.count(), "object"));
                self.clipboard.set(clip);
            }
            None => self.message = "Nothing to copy".to_owned(),
        }
    }

    /// Cut: copy the selection, then erase it.
    fn cut_to_clipboard(&mut self) {
        let ids: Vec<_> = self.selection().iter().copied().collect();
        self.copy_to_clipboard(&ids, None);
        if !ids.is_empty() {
            self.perform(RemoveEntities::new(ids));
            self.message = "Cut to the clipboard".to_owned();
        }
    }

    fn paste_clipboard(&mut self, how: Paste) {
        let Some(clip) = self.clipboard.current() else {
            self.message = "The clipboard is empty".to_owned();
            return;
        };
        if how == Paste::Original {
            let count = plural(clip.count(), "object");
            let placement = Placement::Loose { dx: 0.0, dy: 0.0 };
            self.perform(PasteDrawing::new("Paste", clip.drawing, placement));
            self.message = format!("Pasted {count}");
        } else {
            self.start_command("PASTECLIP", move |ed| paste(ed, clip, how));
        }
    }
}

/// PASTECLIP and PASTEBLOCK: ask where, then bring the drawing in there.
fn paste(ed: &mut Editor, clip: Clipboard, how: Paste) -> Result<(), Cancelled> {
    let shown = clip.clone();
    let options = PointOptions::new("Specify insertion point").ghost(move |at| shown.shifted(at));
    let at = ed.get_point(options)?;
    let count = plural(clip.count(), "object");
    let (name, placement) = match how {
        Paste::Block => {
            let name = ed.read(|doc| doc.unused_block_name())?;
            let block = Placement::Block {
                name: name.clone(),
                base: clip.base,
                at,
            };
            (format!("Paste as {name}"), block)
        }
        _ => (
            "Paste".to_owned(),
            Placement::Loose {
                dx: at.x - clip.base.x,
                dy: at.y - clip.base.y,
            },
        ),
    };
    ed.transaction("Paste", move |tx| {
        tx.perform(PasteDrawing::new(name, clip.drawing, placement));
        Ok(())
    })?;
    ed.say(format!("Pasted {count}"));
    Ok(())
}

/// COPYBASE: pick objects and a base point, and copy them with it.
fn copy_with_base(ed: &mut Editor, slot: ClipboardSlot) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let base = ed.get_point("Specify base point")?;
    match ed.read(move |doc| Clipboard::of(doc, &ids, Some(base)))? {
        Some(clip) => {
            ed.say(format!("Copied {}", plural(clip.count(), "object")));
            slot.set(clip);
        }
        None => ed.say("Nothing to copy"),
    }
    Ok(())
}

/// The command for a Ctrl+C, Ctrl+X or Ctrl+V this frame. egui turns those
/// into events instead of key presses, so the shortcut table never sees them.
pub fn shortcut(ctx: &egui::Context) -> Option<&'static str> {
    ctx.input_mut(|input| {
        let mut found = None;
        input.events.retain(|e| {
            let id = match e {
                egui::Event::Copy => "edit.copy_clip",
                egui::Event::Cut => "edit.cut",
                egui::Event::Paste(_) => "edit.paste",
                _ => return true,
            };
            found = Some(id);
            false
        });
        found
    })
}

fn has_selection(s: &AppState) -> bool {
    !s.selection().is_empty()
}

fn cmd(id: &'static str, label: &'static str, run: fn(&mut AppState)) -> Command<AppState> {
    Command::new(id, label, run)
}

/// The clipboard's commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        cmd("edit.copy_clip", "Copy", |s| {
            let ids: Vec<_> = s.selection().iter().copied().collect();
            s.copy_to_clipboard(&ids, None);
        })
        .key("Ctrl+C")
        .when(has_selection),
        cmd("edit.cut", "Cut", AppState::cut_to_clipboard)
            .key("Ctrl+X")
            .when(has_selection),
        cmd("edit.copy_base", "Copy with Base Point", |s| {
            let slot = s.clipboard.clone();
            s.start_command("COPYBASE", move |ed| copy_with_base(ed, slot))
        })
        .key("Ctrl+Shift+C")
        .transparent(),
        cmd("edit.paste", "Paste", |s| s.paste_clipboard(Paste::At)).key("Ctrl+V"),
        cmd("edit.paste_block", "Paste as Block", |s| {
            s.paste_clipboard(Paste::Block)
        })
        .key("Ctrl+Shift+V"),
        cmd(
            "edit.paste_original",
            "Paste to Original Coordinates",
            |s| s.paste_clipboard(Paste::Original),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(x: f64) -> Entity {
        Entity::Circle {
            center: Point::new(x, 0.0),
            radius: 1.0,
        }
    }

    fn centres(state: &AppState) -> Vec<f64> {
        state
            .session()
            .document()
            .selectable_objects()
            .filter_map(|o| match o.entity {
                Entity::Circle { center, .. } => Some(center.x),
                _ => None,
            })
            .collect()
    }

    fn copy_all(state: &mut AppState) {
        state.select_all();
        let ids: Vec<_> = state.selection().iter().copied().collect();
        state.copy_to_clipboard(&ids, None);
    }

    fn paste_at(state: &mut AppState, how: Paste, at: Point) {
        state.paste_clipboard(how);
        state.settle_command();
        state.answer_point(at);
        state.settle_command();
    }

    #[test]
    fn copy_then_paste_puts_the_lower_left_corner_at_the_point() {
        let mut state = AppState::new();
        state.add_entity(circle(10.0));
        copy_all(&mut state);
        paste_at(&mut state, Paste::At, Point::new(100.0, 0.0));
        // The corner of the circle at 10 is 9; it lands on 100, so the centre on 101.
        assert_eq!(centres(&state), [10.0, 101.0]);
    }

    #[test]
    fn paste_to_original_coordinates_lands_where_it_was_copied() {
        let mut state = AppState::new();
        state.add_entity(circle(10.0));
        copy_all(&mut state);
        state.paste_clipboard(Paste::Original);
        assert_eq!(centres(&state), [10.0, 10.0]);
        state.undo();
        assert_eq!(centres(&state), [10.0]);
    }

    #[test]
    fn paste_as_block_makes_one_reference_to_a_new_block() {
        let mut state = AppState::new();
        state.add_entity(circle(0.0));
        state.add_entity(circle(5.0));
        copy_all(&mut state);
        paste_at(&mut state, Paste::Block, Point::new(50.0, 50.0));
        let document = state.session().document();
        assert_eq!(document.blocks().len(), 1);
        assert_eq!(document.blocks()[0].entities.len(), 2);
        assert_eq!(document.objects().len(), 3);
    }

    #[test]
    fn cut_removes_the_selection_and_paste_brings_it_back() {
        let mut state = AppState::new();
        state.add_entity(circle(0.0));
        state.select_all();
        state.cut_to_clipboard();
        assert!(centres(&state).is_empty());
        paste_at(&mut state, Paste::At, Point::new(-1.0, -1.0));
        assert_eq!(centres(&state), [0.0]);
    }

    #[test]
    fn a_drawing_copied_in_another_window_pastes_here() {
        let mut state = AppState::new();
        state.add_entity(circle(10.0));
        copy_all(&mut state);
        let text = system::read().unwrap();
        // Another window's copy is on the system clipboard, and this one has
        // never copied anything.
        let mut other = AppState::new();
        system::write(&text);
        other.paste_clipboard(Paste::Original);
        assert_eq!(centres(&other), [10.0]);
    }

    #[test]
    fn text_from_another_program_leaves_the_last_copy_in_place() {
        let mut state = AppState::new();
        state.add_entity(circle(10.0));
        copy_all(&mut state);
        system::write("just some words");
        state.paste_clipboard(Paste::Original);
        assert_eq!(centres(&state), [10.0, 10.0]);
    }

    #[test]
    fn an_empty_clipboard_says_so() {
        let mut state = AppState::new();
        system::write("");
        state.paste_clipboard(Paste::At);
        assert_eq!(state.message, "The clipboard is empty");
        assert!(!state.command_running());
    }

    #[test]
    fn pasting_into_another_drawing_works() {
        let mut state = AppState::new();
        state.add_entity(circle(0.0));
        copy_all(&mut state);
        state.new_document();
        state.paste_clipboard(Paste::Original);
        assert_eq!(centres(&state), [0.0]);
    }
}
