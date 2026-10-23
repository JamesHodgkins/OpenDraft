//! The working copy the Text Style dialog (STYLE) edits: the style table, the
//! current style and the selected one. Nothing here touches the drawing until
//! `AppState::set_text_styles` commits it as one undo step.

use od_commands::SetTextStyles;
use od_core::text_style::MAX_NAME_BYTES;
use od_core::{Document, TextStyle, TextStyleId};

use crate::state::AppState;

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub styles: Vec<TextStyle>,
    pub current: TextStyleId,
    pub next_id: u64,
    pub selected: TextStyleId,
    /// The font name as typed for a style, which can end in a space that the
    /// style does not keep.
    pub font_text: Option<(TextStyleId, String)>,
}

impl Draft {
    pub fn from_document(document: &Document) -> Self {
        Self {
            font_text: None,
            styles: document.text_styles().to_vec(),
            current: document.current_text_style(),
            next_id: document.next_text_style_id(),
            selected: document.current_text_style(),
        }
    }

    pub fn selected_style(&mut self) -> Option<&mut TextStyle> {
        let id = self.selected;
        self.styles.iter_mut().find(|s| s.id == id)
    }

    /// `style1`, `style2`... the first name no style has.
    pub fn suggested_name(&self) -> String {
        (1..)
            .map(|n| format!("style{n}"))
            .find(|name| self.name_problem(name).is_none())
            .unwrap_or_default()
    }

    /// Why `name` cannot be a new style's name, if it cannot.
    pub fn name_problem(&self, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() {
            Some("Type a name.".to_owned())
        } else if name.len() > MAX_NAME_BYTES {
            Some("That name is too long.".to_owned())
        } else if self
            .styles
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name))
        {
            Some(format!("A text style named {name} already exists."))
        } else {
            None
        }
    }

    /// Add a style with the stock settings and select it.
    pub fn add(&mut self, name: &str) -> Result<(), String> {
        if let Some(problem) = self.name_problem(name) {
            return Err(problem);
        }
        let id = TextStyleId(self.next_id);
        self.next_id += 1;
        self.styles.push(TextStyle::new(id, name.trim()));
        self.selected = id;
        Ok(())
    }

    /// Standard, the current style and a style in use cannot be deleted.
    pub fn can_delete(&self, document: &Document, id: TextStyleId) -> bool {
        id != TextStyleId::STANDARD
            && id != self.current
            && self.styles.iter().any(|s| s.id == id)
            && !document.text_style_in_use(id)
    }

    pub fn delete(&mut self, id: TextStyleId) {
        self.styles.retain(|s| s.id != id);
        if self.selected == id {
            self.selected = self.current;
        }
    }

    pub fn changed(&self, document: &Document) -> bool {
        self.styles != document.text_styles()
            || self.current != document.current_text_style()
            || self.next_id != document.next_text_style_id()
    }

    pub fn is_valid(&self) -> bool {
        self.styles.iter().all(TextStyle::is_valid)
    }
}

impl AppState {
    /// Replace the text styles as one undo step (the Text Style dialog's
    /// Apply); nothing is recorded when the draft is as the drawing is.
    pub fn set_text_styles(&mut self, draft: &Draft) {
        if !draft.is_valid() || !draft.changed(self.session().document()) {
            return;
        }
        self.perform(SetTextStyles::new(
            draft.styles.clone(),
            draft.current,
            draft.next_id,
        ));
        self.message = "Text styles changed".to_owned();
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, MText, Point};

    use super::*;

    #[test]
    fn new_styles_get_unused_names_and_ids_and_are_selected() {
        let doc = Document::default();
        let mut draft = Draft::from_document(&doc);
        assert_eq!(draft.suggested_name(), "style1");
        draft.add("style1").unwrap();
        assert_eq!(draft.suggested_name(), "style2");
        assert_ne!(draft.selected, TextStyleId::STANDARD);
        assert!(draft.add("STYLE1").is_err(), "names ignore case");
        assert!(draft.add("  ").is_err());
        assert!(draft.changed(&doc));
    }

    #[test]
    fn standard_the_current_and_styles_in_use_stay() {
        let mut doc = Document::default();
        let used = doc.allocate_text_style_id();
        doc.push_text_style(TextStyle::new(used, "Used"));
        doc.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "x",
            used,
        ))));
        let mut draft = Draft::from_document(&doc);
        draft.add("Free").unwrap();
        let free = draft.selected;
        assert!(!draft.can_delete(&doc, TextStyleId::STANDARD));
        assert!(!draft.can_delete(&doc, used));
        assert!(draft.can_delete(&doc, free));
        draft.current = free;
        assert!(!draft.can_delete(&doc, free), "the current one stays");
        draft.current = TextStyleId::STANDARD;
        draft.delete(free);
        assert!(draft.styles.iter().all(|s| s.id != free));
        assert_eq!(draft.selected, TextStyleId::STANDARD);
    }

    #[test]
    fn applying_is_one_undo_step_and_an_unchanged_draft_does_nothing() {
        let mut state = AppState::new();
        let mut draft = Draft::from_document(state.session().document());
        state.set_text_styles(&draft);
        assert!(!state.session().is_modified());
        draft.add("Notes").unwrap();
        draft.current = draft.selected;
        if let Some(style) = draft.selected_style() {
            style.width_factor = 0.8;
        }
        state.set_text_styles(&draft);
        let doc = state.session().document();
        assert_eq!(doc.text_styles().len(), 2);
        assert_eq!(doc.current_text_style(), draft.current);
        state.undo();
        assert_eq!(state.session().document().text_styles().len(), 1);
    }

    #[test]
    fn an_invalid_style_is_not_applied() {
        let mut state = AppState::new();
        let mut draft = Draft::from_document(state.session().document());
        draft.add("Bad").unwrap();
        if let Some(style) = draft.selected_style() {
            style.width_factor = 0.0;
        }
        state.set_text_styles(&draft);
        assert_eq!(state.session().document().text_styles().len(), 1);
    }
}
