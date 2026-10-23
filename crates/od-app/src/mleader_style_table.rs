//! The drawing's named multileader styles: what the Multileader Style
//! Manager does to them. Every operation is one undo step.

use od_commands::SetMLeaderStyles;
use od_core::text_style::MAX_NAME_BYTES;
use od_core::{Entity, EntityId, MLeaderStyle, MLeaderStyleId};

use crate::state::AppState;

/// Which style the Modify Multileader Style dialog is editing.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum MLeaderEdit {
    /// The current style.
    #[default]
    Current,
    /// A style of the table.
    Named(MLeaderStyleId),
    /// A style that is not in the table yet: OK adds it.
    New(Box<MLeaderStyle>),
}

/// Which multileader-style dialogs are open.
#[derive(Default)]
pub struct Dialogs {
    /// The Multileader Style Manager.
    pub manager: bool,
    /// The Modify Multileader Style dialog.
    pub style: bool,
    /// What the Modify dialog edits.
    pub edit: MLeaderEdit,
}

/// Why `name` cannot be a style's name, if it cannot. `ignoring` is the
/// style being renamed, whose own name is free.
pub fn name_problem(
    styles: &[MLeaderStyle],
    name: &str,
    ignoring: Option<MLeaderStyleId>,
) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        Some("Type a name.".to_owned())
    } else if name.len() > MAX_NAME_BYTES {
        Some("That name is too long.".to_owned())
    } else if styles
        .iter()
        .any(|s| s.name.eq_ignore_ascii_case(name) && Some(s.id) != ignoring)
    {
        Some(format!("A multileader style named {name} already exists."))
    } else {
        None
    }
}

/// `Style1`, `Style2`... the first name no style has.
pub fn suggested_name(styles: &[MLeaderStyle]) -> String {
    (1..)
        .map(|n| format!("Style{n}"))
        .find(|name| name_problem(styles, name, None).is_none())
        .unwrap_or_default()
}

impl AppState {
    pub fn mleader_styles(&self) -> Vec<MLeaderStyle> {
        self.session().document().mleader_styles().to_vec()
    }

    /// The style new multileaders are made with.
    pub fn mleader_style(&self) -> MLeaderStyle {
        let doc = self.session().document();
        doc.mleader_style(doc.current_mleader_style())
            .cloned()
            .unwrap_or_else(MLeaderStyle::standard)
    }

    /// Replace the table, as one undo step.
    fn commit_mleader_styles(
        &mut self,
        what: &str,
        table: Vec<MLeaderStyle>,
        current: MLeaderStyleId,
        next_id: u64,
        edits: Vec<(EntityId, Entity)>,
    ) {
        self.perform(SetMLeaderStyles::new(what, table, current, next_id, edits));
    }

    /// Make the style `id` the one new multileaders are made with.
    pub fn set_current_mleader_style(&mut self, id: MLeaderStyleId) {
        let doc = self.session().document();
        if doc.current_mleader_style() == id {
            return;
        }
        let Some(style) = doc.mleader_style(id) else {
            return;
        };
        let name = style.name.clone();
        let (table, next) = (doc.mleader_styles().to_vec(), doc.next_mleader_style_id());
        self.commit_mleader_styles("Set current multileader style", table, id, next, Vec::new());
        self.message = format!("Current multileader style is {name}");
    }

    /// Whether `style` could go in the table: it could exist and writes in a
    /// text style the drawing has.
    fn usable(&self, style: &MLeaderStyle) -> Result<(), String> {
        let doc = self.session().document();
        if doc.text_style(style.content.text_style).is_none() {
            Err("The text style no longer exists.".to_owned())
        } else if !style.is_valid() {
            Err("The style has a value that is out of range.".to_owned())
        } else {
            Ok(())
        }
    }

    /// Add a style to the table with the next free id (not made current).
    pub fn add_mleader_style(&mut self, style: MLeaderStyle) -> Result<(), String> {
        let doc = self.session().document();
        if let Some(problem) = name_problem(doc.mleader_styles(), &style.name, None) {
            return Err(problem);
        }
        let id = MLeaderStyleId(doc.next_mleader_style_id());
        let style = MLeaderStyle {
            id,
            name: style.name.trim().to_owned(),
            ..style
        };
        self.usable(&style)?;
        let doc = self.session().document();
        let mut table = doc.mleader_styles().to_vec();
        let name = style.name.clone();
        table.push(style);
        self.commit_mleader_styles(
            "New multileader style",
            table,
            doc.current_mleader_style(),
            id.0 + 1,
            Vec::new(),
        );
        self.message = format!("Multileader style {name} created");
        Ok(())
    }

    /// Change style `id` to `style`, keeping its id and name.
    pub fn modify_mleader_style(&mut self, id: MLeaderStyleId, style: MLeaderStyle) {
        let doc = self.session().document();
        let Some(slot) = doc.mleader_styles().iter().position(|s| s.id == id) else {
            return;
        };
        let style = MLeaderStyle {
            id,
            name: doc.mleader_styles()[slot].name.clone(),
            ..style
        };
        if let Err(problem) = self.usable(&style) {
            self.message = problem;
            return;
        }
        let doc = self.session().document();
        let mut table = doc.mleader_styles().to_vec();
        let edits = doc.restyled_mleaders(&table[slot].name, &style);
        if table[slot] == style && edits.is_empty() {
            return;
        }
        table[slot] = style;
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        self.commit_mleader_styles("Modify multileader style", table, current, next, edits);
        self.message = "Multileader style changed".to_owned();
    }

    /// Give a style another name. Standard stays.
    pub fn rename_mleader_style(&mut self, id: MLeaderStyleId, to: &str) -> Result<(), String> {
        let doc = self.session().document();
        if id == MLeaderStyleId::STANDARD {
            return Err("Standard cannot be renamed.".to_owned());
        }
        if let Some(problem) = name_problem(doc.mleader_styles(), to, Some(id)) {
            return Err(problem);
        }
        let mut table = doc.mleader_styles().to_vec();
        let Some(style) = table.iter_mut().find(|s| s.id == id) else {
            return Err("There is no such multileader style.".to_owned());
        };
        let from = std::mem::replace(&mut style.name, to.trim().to_owned());
        let edits = doc.restyled_mleaders(&from, style);
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        self.commit_mleader_styles("Rename multileader style", table, current, next, edits);
        Ok(())
    }

    /// Whether a style can be deleted: not Standard, not the current one and
    /// not one a multileader was made in.
    pub fn can_delete_mleader_style(&self, id: MLeaderStyleId) -> bool {
        let doc = self.session().document();
        id != MLeaderStyleId::STANDARD
            && doc.current_mleader_style() != id
            && doc
                .mleader_style(id)
                .is_some_and(|s| !doc.mleader_style_in_use(&s.name))
    }

    pub fn delete_mleader_style(&mut self, id: MLeaderStyleId) -> Result<(), String> {
        if !self.can_delete_mleader_style(id) {
            return Err(
                "Standard, the current style and a style that multileaders use cannot be deleted."
                    .to_owned(),
            );
        }
        let doc = self.session().document();
        let mut table = doc.mleader_styles().to_vec();
        table.retain(|s| s.id != id);
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        self.commit_mleader_styles("Delete multileader style", table, current, next, Vec::new());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes() -> MLeaderStyle {
        MLeaderStyle {
            arrow_size: 4.0,
            ..MLeaderStyle::new(MLeaderStyleId(99), "Notes")
        }
    }

    #[test]
    fn names_are_checked_and_suggested() {
        let styles = vec![MLeaderStyle::standard(), notes()];
        assert!(name_problem(&styles, "NOTES", None).is_some());
        assert!(name_problem(&styles, "NOTES", Some(MLeaderStyleId(99))).is_none());
        assert!(name_problem(&styles, " ", None).is_some());
        assert_eq!(suggested_name(&styles), "Style1");
    }

    #[test]
    fn a_new_style_gets_the_next_id_and_current_is_chosen_by_id() {
        let mut state = AppState::new();
        state.add_mleader_style(notes()).unwrap();
        assert!(state.add_mleader_style(notes()).is_err());
        let styles = state.mleader_styles();
        assert_eq!(styles.len(), 2);
        assert_eq!(styles[1].id, MLeaderStyleId(1));
        assert_eq!(state.mleader_style().name, "Standard");
        state.set_current_mleader_style(MLeaderStyleId(1));
        assert_eq!(state.mleader_style().arrow_size, 4.0);
        state.undo();
        assert_eq!(state.mleader_style().name, "Standard");
        state.undo();
        assert_eq!(state.mleader_styles().len(), 1);
        assert!(state.session().document().validate().is_ok());
    }

    #[test]
    fn modifying_keeps_the_id_and_name_and_is_one_undo_step() {
        let mut state = AppState::new();
        state.add_mleader_style(notes()).unwrap();
        let id = MLeaderStyleId(1);
        state.modify_mleader_style(
            id,
            MLeaderStyle {
                name: "Ignored".into(),
                id: MLeaderStyleId(7),
                arrow_size: 6.0,
                ..notes()
            },
        );
        let doc = state.session().document();
        let style = doc.mleader_style(id).unwrap();
        assert_eq!((style.name.as_str(), style.arrow_size), ("Notes", 6.0));
        state.undo();
        let doc = state.session().document();
        assert_eq!(doc.mleader_style(id).unwrap().arrow_size, 4.0);
    }

    #[test]
    fn a_style_that_cannot_exist_is_refused_and_changes_nothing() {
        let mut state = AppState::new();
        let bad = MLeaderStyle {
            arrow_size: f64::NAN,
            ..notes()
        };
        assert!(state.add_mleader_style(bad.clone()).is_err());
        state.modify_mleader_style(MLeaderStyleId::STANDARD, bad);
        assert_eq!(state.mleader_style(), MLeaderStyle::standard());
        assert!(state.undo().is_none(), "nothing was recorded");
    }

    #[test]
    fn standard_and_the_current_style_stay_and_others_rename_and_delete() {
        let mut state = AppState::new();
        state.add_mleader_style(notes()).unwrap();
        let id = MLeaderStyleId(1);
        assert!(!state.can_delete_mleader_style(MLeaderStyleId::STANDARD));
        assert!(state
            .rename_mleader_style(MLeaderStyleId::STANDARD, "X")
            .is_err());
        assert!(state.rename_mleader_style(id, "standard").is_err());
        state.rename_mleader_style(id, " Callouts ").unwrap();
        assert_eq!(state.mleader_styles()[1].name, "Callouts");
        state.set_current_mleader_style(id);
        assert!(state.delete_mleader_style(id).is_err(), "it is current");
        state.set_current_mleader_style(MLeaderStyleId::STANDARD);
        state.delete_mleader_style(id).unwrap();
        assert_eq!(state.mleader_styles().len(), 1);
        assert!(state.session().document().validate().is_ok());
    }
}
