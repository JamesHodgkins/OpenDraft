//! The drawing's named dimension styles: what the Dimension Style Manager
//! does to them. Every operation is one undo step; editing or renaming a
//! style also restyles the dimensions that were made in it.

use od_commands::SetDimStyles;
use od_core::text_style::MAX_NAME_BYTES;
use od_core::DimStyle;

use crate::state::AppState;

/// Which style the Modify Dimension Style dialog is editing.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum DimEdit {
    /// The current style.
    #[default]
    Current,
    /// A style of the table, by name.
    Named(String),
    /// A style that is not in the table yet: OK adds it.
    New(Box<DimStyle>),
}

/// Why `name` cannot be a style's name, if it cannot. `ignoring` is the
/// style being renamed, whose own name is free.
pub fn name_problem(styles: &[DimStyle], name: &str, ignoring: Option<&str>) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        Some("Type a name.".to_owned())
    } else if name.len() > MAX_NAME_BYTES {
        Some("That name is too long.".to_owned())
    } else if styles.iter().any(|s| {
        s.name.eq_ignore_ascii_case(name) && ignoring.is_none_or(|o| !o.eq_ignore_ascii_case(name))
    }) {
        Some(format!("A dimension style named {name} already exists."))
    } else {
        None
    }
}

/// `Style1`, `Style2`... the first name no style has.
pub fn suggested_name(styles: &[DimStyle]) -> String {
    (1..)
        .map(|n| format!("Style{n}"))
        .find(|name| name_problem(styles, name, None).is_none())
        .unwrap_or_default()
}

fn is_standard(name: &str) -> bool {
    name.eq_ignore_ascii_case("Standard")
}

impl AppState {
    pub fn dim_styles(&self) -> Vec<DimStyle> {
        self.session().document().dim_styles().to_vec()
    }

    /// Make the style called `name` the one new dimensions are made with.
    pub fn set_current_dim_style(&mut self, name: &str) {
        let doc = self.session().document();
        let Some(style) = doc.dim_style_named(name).cloned() else {
            return;
        };
        if style == *doc.dim_style() {
            return;
        }
        let table = doc.dim_styles().to_vec();
        self.perform(SetDimStyles::new(
            "Set current dimension style",
            table,
            style,
            Vec::new(),
        ));
        self.message = format!("Current dimension style is {name}");
    }

    /// Add a style to the table (not made current).
    pub fn add_dim_style(&mut self, style: DimStyle) -> Result<(), String> {
        let doc = self.session().document();
        if let Some(problem) = name_problem(doc.dim_styles(), &style.name, None) {
            return Err(problem);
        }
        let style = style.sanitized();
        let mut table = doc.dim_styles().to_vec();
        table.push(style.clone());
        let current = doc.dim_style().clone();
        self.perform(SetDimStyles::new(
            "New dimension style",
            table,
            current,
            Vec::new(),
        ));
        self.message = format!("Dimension style {} created", style.name);
        Ok(())
    }

    /// Change the style called `name` to `style` (keeping the name), and
    /// every dimension made in it with it.
    pub fn modify_dim_style(&mut self, name: &str, style: DimStyle) {
        let doc = self.session().document();
        let Some(slot) = doc
            .dim_styles()
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
        else {
            return;
        };
        let style = DimStyle {
            name: doc.dim_styles()[slot].name.clone(),
            ..style
        }
        .sanitized();
        let edits = doc.restyled_dimensions(name, &style);
        let mut table = doc.dim_styles().to_vec();
        table[slot] = style.clone();
        let current = if doc.dim_style().name.eq_ignore_ascii_case(name) {
            style
        } else {
            doc.dim_style().clone()
        };
        if table == doc.dim_styles() && current == *doc.dim_style() && edits.is_empty() {
            return;
        }
        self.perform(SetDimStyles::new(
            "Modify dimension style",
            table,
            current,
            edits,
        ));
        self.message = "Dimension style changed".to_owned();
    }

    /// Give a style another name; its dimensions follow. Standard stays.
    pub fn rename_dim_style(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        let doc = self.session().document();
        if is_standard(from) {
            return Err("Standard cannot be renamed.".to_owned());
        }
        if let Some(problem) = name_problem(doc.dim_styles(), to, Some(from)) {
            return Err(problem);
        }
        let Some(slot) = doc
            .dim_styles()
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(from))
        else {
            return Err(format!("There is no dimension style named {from}."));
        };
        let renamed = DimStyle {
            name: to.to_owned(),
            ..doc.dim_styles()[slot].clone()
        };
        let edits = doc.restyled_dimensions(from, &renamed);
        let mut table = doc.dim_styles().to_vec();
        table[slot] = renamed.clone();
        let current = if doc.dim_style().name.eq_ignore_ascii_case(from) {
            renamed
        } else {
            doc.dim_style().clone()
        };
        self.perform(SetDimStyles::new(
            "Rename dimension style",
            table,
            current,
            edits,
        ));
        Ok(())
    }

    /// Whether a style can be deleted: not Standard, not the current one and
    /// not one a dimension was made in.
    pub fn can_delete_dim_style(&self, name: &str) -> bool {
        let doc = self.session().document();
        !is_standard(name)
            && !doc.dim_style().name.eq_ignore_ascii_case(name)
            && doc.dim_style_named(name).is_some()
            && !doc.dim_style_in_use(name)
    }

    pub fn delete_dim_style(&mut self, name: &str) -> Result<(), String> {
        if !self.can_delete_dim_style(name) {
            return Err(
                "Standard, the current style and a style that dimensions use cannot be deleted."
                    .to_owned(),
            );
        }
        let doc = self.session().document();
        let mut table = doc.dim_styles().to_vec();
        table.retain(|s| !s.name.eq_ignore_ascii_case(name));
        let current = doc.dim_style().clone();
        self.perform(SetDimStyles::new(
            "Delete dimension style",
            table,
            current,
            Vec::new(),
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Dimension, Entity, Point};

    use super::*;

    fn metric() -> DimStyle {
        DimStyle {
            name: "Metric".to_owned(),
            text_height: 3.5,
            ..DimStyle::default()
        }
    }

    fn dimension(state: &AppState, name: &str) -> DimStyle {
        let doc = state.session().document();
        doc.objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Dimension(d) if d.style.name == name => Some(d.style.clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn names_are_checked_and_suggested() {
        let styles = vec![DimStyle::default(), metric()];
        assert!(name_problem(&styles, "metric", None).is_some());
        assert!(name_problem(&styles, "metric", Some("Metric")).is_none());
        assert!(name_problem(&styles, " ", None).is_some());
        assert_eq!(suggested_name(&styles), "Style1");
    }

    #[test]
    fn a_new_style_joins_the_table_and_current_is_chosen_by_name() {
        let mut state = AppState::new();
        state.add_dim_style(metric()).unwrap();
        assert!(state.add_dim_style(metric()).is_err());
        assert_eq!(state.dim_styles().len(), 2);
        assert_eq!(state.dim_style().name, "Standard");
        state.set_current_dim_style("metric");
        assert_eq!(state.dim_style().text_height, 3.5);
        state.undo();
        assert_eq!(state.dim_style().name, "Standard");
        state.undo();
        assert_eq!(state.dim_styles().len(), 1);
        assert!(state.session().document().validate().is_ok());
    }

    #[test]
    fn modifying_a_style_restyles_its_dimensions_in_one_step() {
        let mut state = AppState::new();
        state.add_dim_style(metric()).unwrap();
        state.set_current_dim_style("Metric");
        state.add_entity(Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &metric(),
        ))));
        let bigger = DimStyle {
            text_height: 6.0,
            ..metric()
        };
        state.modify_dim_style("Metric", bigger);
        assert_eq!(dimension(&state, "Metric").text_height, 6.0);
        assert_eq!(state.dim_style().text_height, 6.0, "the current style too");
        state.undo();
        assert_eq!(dimension(&state, "Metric").text_height, 3.5);
        assert_eq!(state.dim_style().text_height, 3.5);
    }

    #[test]
    fn rename_and_delete_follow_the_rules() {
        let mut state = AppState::new();
        state.add_dim_style(metric()).unwrap();
        assert!(state.rename_dim_style("Standard", "X").is_err());
        assert!(state.rename_dim_style("Metric", "standard").is_err());
        state.rename_dim_style("Metric", "ISO").unwrap();
        assert!(state.session().document().dim_style_named("ISO").is_some());
        assert!(state.can_delete_dim_style("ISO"));
        assert!(!state.can_delete_dim_style("Standard"));
        state.delete_dim_style("ISO").unwrap();
        assert_eq!(state.dim_styles().len(), 1);

        // A style a dimension uses, or the current one, stays.
        state.add_dim_style(metric()).unwrap();
        state.add_entity(Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &metric(),
        ))));
        assert!(!state.can_delete_dim_style("Metric"));
        state.rename_dim_style("Metric", "Used").unwrap();
        assert_eq!(dimension(&state, "Used").name, "Used");
        assert!(state.session().document().validate().is_ok());
    }
}
