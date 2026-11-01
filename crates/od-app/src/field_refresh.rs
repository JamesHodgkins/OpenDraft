//! `FIELDEVAL`: bringing a drawing's data fields up to date when it is
//! opened, saved or plotted, so a date or a title in a title block is not
//! left showing what it showed when the field was made.
//!
//! Opening changes the drawing as it is read, before it has a history, so it
//! is not a change and does not make the drawing unsaved. Saving and
//! plotting make the change one undo step, "Update fields", and only when
//! something changed. A drawing a command is running on is left alone: the
//! command is working on what it was given.

use std::path::Path;

use od_commands::ModifyEntities;
use od_core::block::{update_all_fields, FieldContext};
use od_core::Document;

use crate::field_context::current_time;
use crate::state::AppState;

/// The moments `FIELDEVAL` names, as its bits.
pub(crate) const ON_OPEN: u8 = 1;
pub(crate) const ON_SAVE: u8 = 2;
pub(crate) const ON_PLOT: u8 = 4;

impl AppState {
    /// Update the fields of drawing `index` if `FIELDEVAL` asks for it at
    /// `moment`. Returns how many fields changed.
    pub(crate) fn refresh_fields(&mut self, index: usize, moment: u8) -> usize {
        if self.settings.field_eval & moment == 0
            || (self.command_running() && index == self.run_doc)
        {
            return 0;
        }
        let context = self.field_context_for(index);
        let Some(doc) = self.docs.get_mut(index) else {
            return 0;
        };
        let update = update_all_fields(doc.session.document(), &context);
        if update.edits.is_empty() {
            return 0;
        }
        doc.session
            .perform(ModifyEntities::new("Update fields", update.edits));
        update.changed
    }

    /// Update the fields of a drawing just read from `path`, before it is
    /// shown, if `FIELDEVAL` asks for it on open.
    pub(crate) fn refresh_loaded(&self, document: &mut Document, path: &Path) {
        if self.settings.field_eval & ON_OPEN == 0 {
            return;
        }
        let context = FieldContext {
            file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
            now: Some(current_time()),
            properties: document.properties().clone(),
        };
        for (id, entity) in update_all_fields(document, &context).edits {
            document.replace(id, entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::block::DataField;
    use od_core::drawing_properties::DrawingProperties;
    use od_core::{Entity, MText, Point, TextStyleId};

    use super::*;
    use crate::picker::{Script, ScriptedPicker};

    fn titled(title: &str) -> DrawingProperties {
        DrawingProperties {
            title: title.into(),
            ..DrawingProperties::default()
        }
    }

    /// A drawing titled "Plot 7" holding a text that shows its title.
    fn with_title_text() -> AppState {
        let mut state = AppState::new();
        state.set_drawing_properties(titled("Plot 7"));
        let mut text = MText::new(Point::new(0.0, 0.0), 2.5, "", TextStyleId::STANDARD);
        let field = DataField::variable("Title", "")
            .evaluated(state.session().document(), &state.field_context());
        text.insert_field(0, field);
        state.add_entity(Entity::MText(Box::new(text)));
        state
    }

    fn shown(state: &AppState, index: usize) -> Vec<String> {
        state.docs[index]
            .session
            .document()
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::MText(t) => Some(t.field_values()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn a_stale_field_is_updated_as_one_undo_step_only_when_it_changed() {
        let mut state = with_title_text();
        state.set_drawing_properties(titled("Plot 8"));
        assert_eq!(
            shown(&state, 0),
            ["Plot 7"],
            "stale until something updates"
        );
        assert_eq!(state.refresh_fields(0, ON_SAVE), 1);
        assert_eq!(shown(&state, 0), ["Plot 8"]);
        let before = state.session().revision();
        assert_eq!(state.refresh_fields(0, ON_SAVE), 0);
        assert_eq!(
            state.session().revision(),
            before,
            "nothing changed, no step"
        );
        state.undo();
        assert_eq!(shown(&state, 0), ["Plot 7"], "one undo step");
    }

    #[test]
    fn fieldeval_chooses_the_moments() {
        let mut state = with_title_text();
        state.set_drawing_properties(titled("Plot 8"));
        state.settings.field_eval = ON_OPEN | ON_PLOT;
        assert_eq!(state.refresh_fields(0, ON_SAVE), 0, "save is off");
        state.settings.field_eval = 0;
        assert_eq!(state.refresh_fields(0, ON_PLOT), 0, "everything is off");
        state.settings.field_eval = ON_SAVE;
        assert_eq!(state.refresh_fields(0, ON_SAVE), 1);
    }

    #[test]
    fn a_drawing_a_command_is_running_on_is_left_alone() {
        let mut state = with_title_text();
        state.set_drawing_properties(titled("Plot 8"));
        state.start_command("LINE", crate::draw::draw_line);
        state.settle_command();
        assert!(state.command_running());
        assert_eq!(state.refresh_fields(0, ON_SAVE), 0);
        assert_eq!(shown(&state, 0), ["Plot 7"]);
    }

    #[test]
    fn saving_writes_the_fields_as_they_are_now() {
        let dir = std::env::temp_dir().join(format!("od-fieldeval-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plan.odx");
        let mut state = with_title_text();
        state.set_drawing_properties(titled("Plot 8"));
        let script = Script::default();
        state.picker = Box::new(ScriptedPicker(script.clone()));
        script.will_save_to(Some(path.clone()));
        assert!(state.save_active());
        state.finish_io();
        let written = od_files::load(&path).unwrap().document;
        let text = written.objects().iter().find_map(|o| match &o.entity {
            Entity::MText(t) => Some(t.field_values()),
            _ => None,
        });
        assert_eq!(text, Some(vec!["Plot 8".to_owned()]));
        assert!(!state.docs[0].is_modified(), "saved, so clean");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opening_refreshes_without_making_the_drawing_unsaved() {
        let dir = std::env::temp_dir().join(format!("od-fieldeval-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.odx");
        // A file whose field says "Plot 7" although the drawing is titled
        // "Plot 9": made as an older save would have left it.
        let mut stale = with_title_text();
        stale.set_drawing_properties(titled("Plot 9"));
        od_files::save(&path, stale.session().document()).unwrap();
        let mut state = AppState::new();
        assert!(state.open_path(&path));
        state.finish_io();
        let index = state.docs.len() - 1;
        assert_eq!(shown(&state, index), ["Plot 9"], "refreshed on open");
        assert!(!state.docs[index].is_modified(), "opening is not a change");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
