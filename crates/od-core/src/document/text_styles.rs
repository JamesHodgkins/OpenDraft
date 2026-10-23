//! The text-style table: styles in order, the one new text is made with, and
//! the counter that keeps ids from being reused.

use super::{after, Document};
use crate::text_style::{TextStyle, TextStyleId};

impl Document {
    /// Text styles in table order. `Standard` is always first.
    pub fn text_styles(&self) -> &[TextStyle] {
        &self.text_styles
    }

    pub fn text_style(&self, id: TextStyleId) -> Option<&TextStyle> {
        self.text_styles.iter().find(|s| s.id == id)
    }

    /// The style called `name`, ignoring ASCII case as DXF does.
    pub fn text_style_named(&self, name: &str) -> Option<&TextStyle> {
        self.text_styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// The style new text is made with.
    pub fn current_text_style(&self) -> TextStyleId {
        self.current_text_style
    }

    /// Make `id` the style new text uses. Ignores unknown ids.
    pub fn set_current_text_style(&mut self, id: TextStyleId) {
        if self.text_style(id).is_some() {
            self.current_text_style = id;
        }
    }

    pub fn allocate_text_style_id(&mut self) -> TextStyleId {
        let id = TextStyleId(self.next_text_style_id);
        self.next_text_style_id = after(self.next_text_style_id);
        id
    }

    /// The next text-style id that would be handed out.
    pub fn next_text_style_id(&self) -> u64 {
        self.next_text_style_id
    }

    /// Make sure ids already used are never handed out again. Only raises it.
    pub fn reserve_text_style_id(&mut self, next: u64) {
        self.next_text_style_id = self.next_text_style_id.max(next);
    }

    /// Append a style (what undo and loading use; the caller keeps ids and
    /// names unique, and `validate` checks).
    pub fn push_text_style(&mut self, style: TextStyle) {
        self.next_text_style_id = self.next_text_style_id.max(after(style.id.0));
        self.text_styles.push(style);
    }

    /// Swap the style with the same id, returning the old one, or `None`
    /// (changing nothing) when there is none.
    pub fn replace_text_style(&mut self, style: TextStyle) -> Option<TextStyle> {
        let slot = self.text_styles.iter_mut().find(|s| s.id == style.id)?;
        Some(std::mem::replace(slot, style))
    }

    /// Whether an object, a block's contents or a dimension style (the
    /// current one or a dimension's own) writes in `id`.
    pub fn text_style_in_use(&self, id: TextStyleId) -> bool {
        use crate::Entity;
        let uses = |entity: &Entity| match entity {
            Entity::MText(m) => m.style == id,
            Entity::MLeader(m) => m.text().is_some_and(|t| t.text.style == id),
            Entity::Dimension(d) => d.style.text_style == id,
            _ => false,
        };
        self.dim_style().text_style == id
            || self.objects().iter().any(|o| uses(&o.entity))
            || self
                .blocks()
                .iter()
                .any(|b| b.entities.iter().any(|e| uses(&e.entity)))
    }

    /// Swap in the whole table, the current style and the id counter,
    /// returning what was there (what undo and the Text Style dialog use).
    /// The caller keeps ids and names unique and `current` in the table.
    pub fn set_text_styles(
        &mut self,
        styles: Vec<TextStyle>,
        current: TextStyleId,
        next_id: u64,
    ) -> (Vec<TextStyle>, TextStyleId, u64) {
        let old = (
            std::mem::replace(&mut self.text_styles, styles),
            self.current_text_style,
            self.next_text_style_id,
        );
        self.current_text_style = current;
        self.next_text_style_id = next_id;
        old
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dimension, Entity, MText, Point};

    #[test]
    fn a_style_is_in_use_by_text_and_by_dimensions() {
        let mut doc = Document::default();
        let notes = doc.allocate_text_style_id();
        doc.push_text_style(TextStyle::new(notes, "Notes"));
        assert!(!doc.text_style_in_use(notes));
        doc.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "x",
            notes,
        ))));
        assert!(doc.text_style_in_use(notes));

        let other = doc.allocate_text_style_id();
        doc.push_text_style(TextStyle::new(other, "Other"));
        let style = crate::DimStyle {
            text_style: other,
            ..crate::DimStyle::default()
        };
        doc.add(Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            &style,
        ))));
        assert!(doc.text_style_in_use(other));
    }

    #[test]
    fn the_table_swaps_whole_and_comes_back() {
        let mut doc = Document::default();
        let notes = doc.allocate_text_style_id();
        let mut table = doc.text_styles().to_vec();
        table.push(TextStyle::new(notes, "Notes"));
        let old = doc.set_text_styles(table, notes, 5);
        assert_eq!(doc.current_text_style(), notes);
        assert_eq!(doc.next_text_style_id(), 5);
        doc.set_text_styles(old.0, old.1, old.2);
        assert_eq!(doc.text_styles().len(), 1);
        assert_eq!(doc.current_text_style(), TextStyleId::STANDARD);
    }
}
