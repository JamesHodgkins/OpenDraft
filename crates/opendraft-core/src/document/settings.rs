//! The drawing's own settings: its units and the dimension style.

use super::Document;
use crate::dimension::DimStyle;
use crate::drawing_properties::DrawingProperties;
use crate::units::Units;
use crate::{Entity, EntityId};

impl Document {
    // ---- units ----

    pub fn units(&self) -> &Units {
        &self.units
    }

    /// Replace the unit settings (with every value brought into range).
    /// Returns the old ones.
    pub fn set_units(&mut self, units: Units) -> Units {
        std::mem::replace(&mut self.units, units.sanitized())
    }

    // ---- drawing properties ----

    pub fn properties(&self) -> &DrawingProperties {
        &self.properties
    }

    /// Replace the drawing's properties (cleaned to the limits). Returns the
    /// old ones.
    pub fn set_properties(&mut self, properties: DrawingProperties) -> DrawingProperties {
        std::mem::replace(&mut self.properties, properties.sanitized())
    }

    // ---- dimension style ----

    /// The style new dimensions are made with (existing ones keep theirs).
    pub fn dim_style(&self) -> &DimStyle {
        &self.dim_style
    }

    /// Replace the current dimension style (brought into range); the table's
    /// style of that name changes with it. Returns the old one.
    pub fn set_dim_style(&mut self, style: DimStyle) -> DimStyle {
        let style = style.sanitized();
        match self
            .dim_styles
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&style.name))
        {
            Some(slot) => *slot = style.clone(),
            None => self.dim_styles.push(style.clone()),
        }
        std::mem::replace(&mut self.dim_style, style)
    }

    /// The named dimension styles, `Standard` first.
    pub fn dim_styles(&self) -> &[DimStyle] {
        &self.dim_styles
    }

    /// The style called `name`, ignoring case.
    pub fn dim_style_named(&self, name: &str) -> Option<&DimStyle> {
        self.dim_styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Swap in the whole table and the current style, returning what was
    /// there (what undo and the Dimension Style Manager use). The caller
    /// keeps names unique and `current` in the table.
    pub fn set_dim_styles(
        &mut self,
        styles: Vec<DimStyle>,
        current: DimStyle,
    ) -> (Vec<DimStyle>, DimStyle) {
        (
            std::mem::replace(&mut self.dim_styles, styles),
            std::mem::replace(&mut self.dim_style, current),
        )
    }

    /// Whether a dimension (in the drawing or in a block) was made in the
    /// style called `name`.
    pub fn dim_style_in_use(&self, name: &str) -> bool {
        let uses = |entity: &Entity| matches!(entity, Entity::Dimension(d) if d.style.name.eq_ignore_ascii_case(name));
        self.objects().iter().any(|o| uses(&o.entity))
            || self
                .blocks()
                .iter()
                .any(|b| b.entities.iter().any(|e| uses(&e.entity)))
    }

    /// The dimensions made in the style called `old_name`, each as it would
    /// be with `new` instead (the way AutoCAD updates dimensions when their
    /// style changes). Dimensions already as `new` are left out.
    pub fn restyled_dimensions(&self, old_name: &str, new: &DimStyle) -> Vec<(EntityId, Entity)> {
        let new = new.clone().sanitized();
        self.objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d)
                    if d.style.name.eq_ignore_ascii_case(old_name) && d.style != new =>
                {
                    let mut changed = (**d).clone();
                    changed.style = new.clone();
                    Some((o.id, Entity::Dimension(Box::new(changed))))
                }
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dimension, Point};

    fn dimension(style: &DimStyle) -> Entity {
        Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            style,
        )))
    }

    #[test]
    fn the_table_starts_with_standard_and_follows_the_current_style() {
        let mut doc = Document::default();
        assert_eq!(doc.dim_styles().len(), 1);
        let tall = DimStyle {
            text_height: 5.0,
            ..DimStyle::default()
        };
        doc.set_dim_style(tall.clone());
        assert_eq!(doc.dim_styles().len(), 1, "same name, same slot");
        assert_eq!(doc.dim_style_named("STANDARD"), Some(&tall));
    }

    #[test]
    fn use_and_restyling_follow_the_style_name() {
        let mut doc = Document::default();
        let metric = DimStyle {
            name: "Metric".into(),
            ..DimStyle::default()
        };
        let a = doc.add(dimension(&metric));
        doc.add(dimension(&DimStyle::default()));
        assert!(doc.dim_style_in_use("metric"));
        assert!(!doc.dim_style_in_use("Other"));
        let bigger = DimStyle {
            text_height: 4.0,
            ..metric
        };
        let edits = doc.restyled_dimensions("Metric", &bigger);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].0, a);
        let Entity::Dimension(d) = &edits[0].1 else {
            panic!("dimension expected");
        };
        assert_eq!(d.style.text_height, 4.0);
        let new_style = d.style.clone();
        doc.replace(a, edits[0].1.clone());
        assert!(doc.restyled_dimensions("Metric", &new_style).is_empty());
    }
}
