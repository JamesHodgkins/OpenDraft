//! How the text editor joins the running command: it opens when a command
//! waits for it, and closes by answering that command or cancelling it.

use od_commands::{Reply, Wants};
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
            .and_then(|ask| ask.mtext.clone());
        let Some(doc) = self.docs.get_mut(self.run_doc) else {
            return;
        };
        match wanted {
            Some(text) if doc.mtext_editor.is_none() => {
                let style = doc
                    .session
                    .document
                    .text_style(text.style)
                    .cloned()
                    .unwrap_or_else(TextStyle::standard);
                doc.mtext_editor = Some(MTextEditor::new(*text, style));
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
                let text = editor.text().clone();
                let keep = editor.is_changed() && !text.contents.trim().is_empty();
                self.answer_mtext(keep.then_some(text));
            }
            Outcome::Cancel => self.cancel_command(),
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
