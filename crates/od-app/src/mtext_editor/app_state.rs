//! How the text editor joins the running command: it opens when a command
//! waits for it, and closes by answering that command or cancelling it.

use od_commands::{Reply, Wants};
use od_core::block::ObjectChoice;
use od_core::{MText, TextStyle};

use super::{MTextEditor, Outcome};
use crate::state::AppState;

impl AppState {
    /// Open or close the in-place text editor to match what the running
    /// command is waiting for.
    pub(crate) fn sync_mtext_editor(&mut self) {
        let wanted = self
            .waiting()
            .filter(|ask| ask.wants == Wants::MTextEditor)
            .and_then(|ask| ask.mtext.clone().map(|text| (text, ask.mtext_of)));
        let fields = self.field_context();
        let Some(doc) = self.docs.get_mut(self.run_doc) else {
            return;
        };
        match wanted {
            Some((text, of)) if doc.mtext_editor.is_none() => {
                let style = doc
                    .session
                    .document()
                    .text_style(text.style)
                    .cloned()
                    .unwrap_or_else(TextStyle::standard);
                let mut editor = MTextEditor::new(*text, style);
                editor.replaces = of;
                editor.field_context = fields;
                doc.mtext_editor = Some(editor);
            }
            None => doc.mtext_editor = None,
            Some(_) => {}
        }
    }

    /// Whether the text editor is open, and so owns the keyboard.
    pub(crate) fn mtext_editing(&self) -> bool {
        self.docs
            .get(self.run_doc)
            .is_some_and(|doc| doc.mtext_editor.is_some())
    }

    /// Close the editor. Accepting hands the text to the command (nothing
    /// when it is empty or unchanged); cancelling cancels the command.
    pub(crate) fn finish_mtext_editor(&mut self, outcome: Outcome) {
        let Some(editor) = self
            .docs
            .get_mut(self.run_doc)
            .and_then(|doc| doc.mtext_editor.take())
        else {
            return;
        };
        match outcome {
            Outcome::Accept => {
                let mut text = editor.text().clone();
                text.prune_fields();
                let keep = editor.is_changed() && !text.contents.trim().is_empty();
                self.answer_mtext(keep.then_some(text));
            }
            Outcome::Cancel => self.cancel_command(),
        }
    }

    /// A click while an object is being picked for a field in the open text
    /// editor: the object becomes the dialog's, which then shows again. A
    /// click on nothing keeps waiting.
    pub(crate) fn answer_field_pick(&mut self, picked: Option<od_core::EntityId>) {
        let context = self.field_context();
        let Some(doc) = self.docs.get_mut(self.run_doc) else {
            return;
        };
        let (Some(editor), Some(id)) = (doc.mtext_editor.as_mut().filter(|e| e.field_pick), picked)
        else {
            return;
        };
        match ObjectChoice::of(doc.session.document(), id, &context) {
            Some(choice) if !choice.properties.is_empty() => {
                editor.field_object = Some(choice);
                editor.field_pick = false;
                editor.notice = None;
            }
            _ => {
                editor.notice = Some("That object has no property a field can show.".into());
            }
        }
    }

    /// Answer the text editor prompt: the edited text, or nothing to keep.
    pub(crate) fn answer_mtext(&mut self, text: Option<MText>) {
        match text {
            Some(text) => self.reply(Reply::MText(Box::new(text))),
            None => self.reply(Reply::Enter),
        }
    }
}
