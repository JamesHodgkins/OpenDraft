//! Changing the style of the selected objects from the Properties panel:
//! the dimension style of dimensions and the text style of text, as one undo
//! step.

use od_commands::ModifyEntities;
use od_core::Entity;

use crate::state::AppState;
use crate::style_rows::StyleEdit;

impl AppState {
    /// Give every selected dimension the dimension style called `name`, or
    /// every selected piece of text the text style `id`. Objects of the other
    /// kind are left alone, and so are those already in it.
    pub fn set_object_style(&mut self, edit: &StyleEdit) {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let document = doc.session.document();
        let label = match edit {
            StyleEdit::Dimension(_) => "dimension style",
            StyleEdit::Text(_) => "text style",
        };
        let mut changes = Vec::new();
        for &id in &doc.selection {
            let next = match (document.get(id), edit) {
                (Some(Entity::Dimension(d)), StyleEdit::Dimension(name)) => {
                    let Some(style) = document.dim_style_named(name) else {
                        continue;
                    };
                    if d.style == *style {
                        continue;
                    }
                    let mut changed = (**d).clone();
                    changed.style = style.clone();
                    Entity::Dimension(Box::new(changed))
                }
                (Some(Entity::MText(m)), StyleEdit::Text(style)) => {
                    if m.style == *style || document.text_style(*style).is_none() {
                        continue;
                    }
                    let mut changed = (**m).clone();
                    changed.style = *style;
                    Entity::MText(Box::new(changed))
                }
                _ => continue,
            };
            changes.push((id, next));
        }
        if changes.is_empty() {
            return;
        }
        self.message = format!("Changed the {label} of {} objects", changes.len());
        doc.session
            .perform(ModifyEntities::new(format!("Change {label}"), changes));
    }
}

#[cfg(test)]
mod tests {
    use od_core::{DimStyle, Dimension, MText, Point, TextStyle, TextStyleId};

    use super::*;

    fn dimension() -> Entity {
        Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &DimStyle::default(),
        )))
    }

    #[test]
    fn selected_dimensions_take_the_chosen_style_and_undo_restores_them() {
        let mut state = AppState::new();
        state
            .add_dim_style(DimStyle {
                name: "Metric".to_owned(),
                text_height: 4.0,
                ..DimStyle::default()
            })
            .unwrap();
        state.add_entity(dimension());
        state.add_entity(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "x",
            TextStyleId::STANDARD,
        ))));
        state.select_all();
        state.set_object_style(&StyleEdit::Dimension("metric".to_owned()));
        let heights = |s: &AppState| -> Vec<f64> {
            s.session()
                .document()
                .objects()
                .iter()
                .filter_map(|o| match &o.entity {
                    Entity::Dimension(d) => Some(d.style.text_height),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(heights(&state), [4.0]);
        state.undo();
        assert_eq!(heights(&state), [2.5]);
    }

    #[test]
    fn selected_text_takes_the_chosen_text_style() {
        let mut state = AppState::new();
        let id = {
            let doc = state.session().document();
            doc.next_text_style_id()
        };
        let mut draft = crate::text_style_draft::Draft::from_document(state.session().document());
        draft.add("Notes").unwrap();
        state.set_text_styles(&draft);
        let notes = state.session().document().text_styles()[1].id;
        assert_eq!(notes.0, id);
        state.add_entity(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "x",
            TextStyleId::STANDARD,
        ))));
        state.select_all();
        state.set_object_style(&StyleEdit::Text(notes));
        let Some(Entity::MText(m)) = state
            .session()
            .document()
            .objects()
            .first()
            .map(|o| &o.entity)
        else {
            panic!("text expected");
        };
        assert_eq!(m.style, notes);
        // A style that is not there changes nothing.
        state.set_object_style(&StyleEdit::Text(TextStyleId(99)));
        let _ = TextStyle::standard();
        assert!(state.session().document().validate().is_ok());
    }
}
