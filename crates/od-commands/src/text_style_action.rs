//! Undoable change of the drawing's text styles: the table, the current
//! style and the id counter.

use od_core::{Document, TextStyle, TextStyleId};

use crate::{Action, Changes};

/// Replace the text-style table (the Text Style dialog).
pub struct SetTextStyles {
    /// Applying and reverting both swap, so each leaves the other state ready.
    styles: Vec<TextStyle>,
    current: TextStyleId,
    next_id: u64,
}

impl SetTextStyles {
    pub fn new(styles: Vec<TextStyle>, current: TextStyleId, next_id: u64) -> Self {
        Self {
            styles,
            current,
            next_id,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        let (styles, current, next_id) =
            doc.set_text_styles(std::mem::take(&mut self.styles), self.current, self.next_id);
        self.styles = styles;
        self.current = current;
        self.next_id = next_id;
    }
}

impl Action for SetTextStyles {
    fn name(&self) -> String {
        "Text styles".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::text_styles()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_changes_and_comes_back() {
        let mut doc = Document::default();
        let notes = doc.allocate_text_style_id();
        let mut table = doc.text_styles().to_vec();
        table.push(TextStyle::new(notes, "Notes"));
        let mut action = SetTextStyles::new(table, notes, doc.next_text_style_id());
        action.apply(&mut doc);
        assert_eq!(doc.text_styles().len(), 2);
        assert_eq!(doc.current_text_style(), notes);
        action.revert(&mut doc);
        assert_eq!(doc.text_styles().len(), 1);
        assert_eq!(doc.current_text_style(), TextStyleId::STANDARD);
        assert!(action.changes().text_styles);
    }
}
