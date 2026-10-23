//! Undoable change of the dimension-style table: the styles, the current one
//! and the dimensions that were made in a style that changed.

use od_core::{DimStyle, Document, Entity, EntityId};

use crate::{Action, Changes};

/// Replace the dimension-style table and restyle some dimensions with it (the
/// Dimension Style Manager).
pub struct SetDimStyles {
    /// Applying and reverting both swap, so each leaves the other state ready.
    styles: Vec<DimStyle>,
    current: DimStyle,
    /// The dimensions to change and what to change them to; after `apply`
    /// they hold what was there before.
    edits: Vec<(EntityId, Entity)>,
    name: String,
}

impl SetDimStyles {
    pub fn new(
        name: impl Into<String>,
        styles: Vec<DimStyle>,
        current: DimStyle,
        edits: Vec<(EntityId, Entity)>,
    ) -> Self {
        Self {
            styles,
            current: current.sanitized(),
            edits,
            name: name.into(),
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        let (styles, current) =
            doc.set_dim_styles(std::mem::take(&mut self.styles), self.current.clone());
        self.styles = styles;
        self.current = current;
        for (id, entity) in &mut self.edits {
            if let Some(old) = doc.replace(*id, entity.clone()) {
                *entity = old;
            }
        }
    }
}

impl Action for SetDimStyles {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes {
            changed: self.edits.iter().map(|(id, _)| *id).collect(),
            dim_style: true,
            ..Changes::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Dimension, Point};

    use super::*;

    #[test]
    fn the_table_and_the_restyled_dimensions_change_and_come_back() {
        let mut doc = Document::default();
        let id = doc.add(Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &DimStyle::default(),
        ))));
        let big = DimStyle {
            text_height: 6.0,
            ..DimStyle::default()
        };
        let edits = doc.restyled_dimensions("Standard", &big);
        let mut action = SetDimStyles::new("Modify", vec![big.clone()], big.clone(), edits);
        action.apply(&mut doc);
        assert_eq!(doc.dim_style().text_height, 6.0);
        let Some(Entity::Dimension(d)) = doc.get(id) else {
            panic!("dimension expected");
        };
        assert_eq!(d.style.text_height, 6.0);
        assert_eq!(action.changes().changed, vec![id]);
        action.revert(&mut doc);
        assert_eq!(doc.dim_style().text_height, 2.5);
        let Some(Entity::Dimension(d)) = doc.get(id) else {
            panic!("dimension expected");
        };
        assert_eq!(d.style.text_height, 2.5);
        assert!(doc.validate().is_ok());
    }
}
