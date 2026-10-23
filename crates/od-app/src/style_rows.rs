//! The style row of the Properties panel: the dimension style of selected
//! dimensions and the text style of selected text, each chosen from the
//! drawing's styles. Shown only when everything shown is of that one kind
//! (as in AutoCAD, which lists the properties the selection has in common).

use eframe::egui::Ui;
use od_core::{Document, Entity, Object, TextStyleId};
use od_ui::Theme;

use crate::attr_rows::{cell_combo, shared, VARIES};
use crate::properties_ui::row_frame;

/// A style chosen in the panel.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleEdit {
    /// A dimension style, by name.
    Dimension(String),
    Text(TextStyleId),
}

/// Which style the shown objects have, when they are all of one kind that
/// has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dimension,
    Text,
}

impl Kind {
    /// The section's title.
    pub fn title(self) -> &'static str {
        match self {
            Kind::Dimension => "Misc",
            Kind::Text => "Text",
        }
    }
}

pub fn kind(objects: &[&Object]) -> Option<Kind> {
    if objects.is_empty() {
        None
    } else if objects
        .iter()
        .all(|o| matches!(o.entity, Entity::Dimension(_)))
    {
        Some(Kind::Dimension)
    } else if objects.iter().all(|o| matches!(o.entity, Entity::MText(_))) {
        Some(Kind::Text)
    } else {
        None
    }
}

/// The row for `objects` (all of `kind`); returns the style picked.
pub fn row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    objects: &[&Object],
    kind: Kind,
) -> Option<StyleEdit> {
    match kind {
        Kind::Dimension => {
            let (_, cell) = row_frame(ui, theme, "Dim style");
            let names: Vec<String> = document
                .dim_styles()
                .iter()
                .map(|s| s.name.clone())
                .collect();
            let name = shared(objects.iter().filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some(d.style.name.to_ascii_lowercase()),
                _ => None,
            }));
            let current = name.and_then(|n| names.iter().position(|s| s.to_ascii_lowercase() == n));
            let text = current.map_or(VARIES, |i| names[i].as_str());
            let id = ui.id().with("style_dimension");
            let picked = cell_combo(ui, theme, id, cell, text, &names, current)?;
            Some(StyleEdit::Dimension(names[picked].clone()))
        }
        Kind::Text => {
            let (_, cell) = row_frame(ui, theme, "Style");
            let styles = document.text_styles();
            let names: Vec<String> = styles.iter().map(|s| s.name.clone()).collect();
            let id_of = shared(objects.iter().filter_map(|o| match &o.entity {
                Entity::MText(m) => Some(m.style),
                _ => None,
            }));
            let current = id_of.and_then(|id| styles.iter().position(|s| s.id == id));
            let text = current.map_or(VARIES, |i| names[i].as_str());
            let id = ui.id().with("style_text");
            let picked = cell_combo(ui, theme, id, cell, text, &names, current)?;
            Some(StyleEdit::Text(styles[picked].id))
        }
    }
}
