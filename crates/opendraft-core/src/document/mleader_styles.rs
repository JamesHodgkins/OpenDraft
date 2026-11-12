//! The multileader-style table: styles in order, the one new multileaders are
//! made with, and the counter that keeps ids from being reused.

use std::collections::HashSet;

use super::Document;
use crate::mleader::{MLeaderStyle, MLeaderStyleId};
use crate::validate::DocumentError;
use crate::{Entity, EntityId};

impl Document {
    /// Multileader styles in table order. `Standard` is always first.
    pub fn mleader_styles(&self) -> &[MLeaderStyle] {
        &self.mleader_styles
    }

    pub fn mleader_style(&self, id: MLeaderStyleId) -> Option<&MLeaderStyle> {
        self.mleader_styles.iter().find(|s| s.id == id)
    }

    /// The style called `name`, ignoring ASCII case as DXF does.
    pub fn mleader_style_named(&self, name: &str) -> Option<&MLeaderStyle> {
        self.mleader_styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// The style new multileaders are made with.
    pub fn current_mleader_style(&self) -> MLeaderStyleId {
        self.current_mleader_style
    }

    /// The next multileader-style id that would be handed out.
    pub fn next_mleader_style_id(&self) -> u64 {
        self.next_mleader_style_id
    }

    /// Make sure ids already used are never handed out again. Only raises it.
    pub fn reserve_mleader_style_id(&mut self, next: u64) {
        self.next_mleader_style_id = self.next_mleader_style_id.max(next);
    }

    /// Swap in the whole table, the current style and the id counter,
    /// returning what was there (what undo and the style dialog use). The
    /// caller keeps ids and names unique and `current` in the table.
    pub fn set_mleader_styles(
        &mut self,
        styles: Vec<MLeaderStyle>,
        current: MLeaderStyleId,
        next_id: u64,
    ) -> (Vec<MLeaderStyle>, MLeaderStyleId, u64) {
        let old = (
            std::mem::replace(&mut self.mleader_styles, styles),
            self.current_mleader_style,
            self.next_mleader_style_id,
        );
        self.current_mleader_style = current;
        self.next_mleader_style_id = next_id;
        old
    }
}

impl Document {
    /// Whether a multileader was made in the style called `name`.
    pub fn mleader_style_in_use(&self, name: &str) -> bool {
        self.objects().iter().any(
            |o| matches!(&o.entity, Entity::MLeader(m) if m.style_name.eq_ignore_ascii_case(name)),
        )
    }

    /// The multileaders made in the style called `old_name`, each as it
    /// would be in `new` instead (what a style change does to them).
    /// Multileaders already as `new` leave out.
    pub fn restyled_mleaders(&self, old_name: &str, new: &MLeaderStyle) -> Vec<(EntityId, Entity)> {
        self.objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MLeader(m) if m.style_name.eq_ignore_ascii_case(old_name) => {
                    let changed = m.restyled(new);
                    (changed != **m).then(|| (o.id, Entity::MLeader(Box::new(changed))))
                }
                _ => None,
            })
            .collect()
    }

    /// Every multileader could exist, writes in a text style the drawing
    /// has and names a style it has.
    pub(crate) fn validate_mleaders(&self) -> Result<(), DocumentError> {
        for object in self.objects() {
            let Entity::MLeader(m) = &object.entity else {
                continue;
            };
            let bad = |why: String| Err(DocumentError::BadMLeader(object.id, why));
            if let Err(e) = m.validate() {
                return bad(e.to_string());
            }
            if self.mleader_style_named(&m.style_name).is_none() {
                return bad(format!("there is no style called {}", m.style_name));
            }
            if m.text()
                .is_some_and(|t| self.text_style(t.text.style).is_none())
            {
                return bad("its text style does not exist".to_owned());
            }
        }
        Ok(())
    }

    /// `Standard` first, ids and names unique, every style one that could
    /// exist and write in a text style that does, the current style in the
    /// table and the counter ahead of every id.
    pub(crate) fn validate_mleader_styles(&self) -> Result<(), DocumentError> {
        let styles = self.mleader_styles();
        if styles.first().map(|s| s.id) != Some(MLeaderStyleId::STANDARD) {
            return Err(DocumentError::BadMLeaderStyle("Standard".into()));
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for style in styles {
            let unique = ids.insert(style.id) && names.insert(style.name.to_ascii_lowercase());
            let text_ok = self.text_style(style.content.text_style).is_some();
            if !unique || !text_ok || !style.is_valid() {
                return Err(DocumentError::BadMLeaderStyle(style.name.clone()));
            }
        }
        if !ids.contains(&self.current_mleader_style()) {
            return Err(DocumentError::CurrentMLeaderStyleMissing(
                self.current_mleader_style(),
            ));
        }
        if styles
            .iter()
            .any(|s| s.id.0 >= self.next_mleader_style_id())
        {
            return Err(DocumentError::CounterBehind);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_drawing_has_only_standard_current() {
        let doc = Document::default();
        assert_eq!(doc.mleader_styles().len(), 1);
        assert_eq!(doc.current_mleader_style(), MLeaderStyleId::STANDARD);
        assert_eq!(doc.next_mleader_style_id(), 1);
        assert!(doc.mleader_style_named("STANDARD").is_some());
    }

    #[test]
    fn the_table_swaps_whole_and_comes_back() {
        let mut doc = Document::default();
        let notes = MLeaderStyleId(doc.next_mleader_style_id());
        let mut table = doc.mleader_styles().to_vec();
        table.push(MLeaderStyle::new(notes, "Notes"));
        let old = doc.set_mleader_styles(table, notes, 5);
        assert_eq!(doc.current_mleader_style(), notes);
        assert_eq!(doc.next_mleader_style_id(), 5);
        assert!(doc.mleader_style(notes).is_some());
        doc.set_mleader_styles(old.0, old.1, old.2);
        assert_eq!(doc.mleader_styles().len(), 1);
        assert_eq!(doc.current_mleader_style(), MLeaderStyleId::STANDARD);
    }

    #[test]
    fn the_table_is_checked() {
        let mut doc = Document::default();
        assert_eq!(doc.validate(), Ok(()));
        let id = MLeaderStyleId(doc.next_mleader_style_id());
        let mut table = doc.mleader_styles().to_vec();
        table.push(MLeaderStyle::new(id, "STANDARD"));
        doc.set_mleader_styles(table.clone(), id, 2);
        assert!(matches!(
            doc.validate(),
            Err(DocumentError::BadMLeaderStyle(_))
        ));
        table[1].name = "Notes".into();
        doc.set_mleader_styles(table.clone(), MLeaderStyleId(9), 2);
        assert!(matches!(
            doc.validate(),
            Err(DocumentError::CurrentMLeaderStyleMissing(_))
        ));
        doc.set_mleader_styles(table.clone(), id, 1);
        assert_eq!(doc.validate(), Err(DocumentError::CounterBehind));
        table[1].content.text_style = crate::TextStyleId(77);
        doc.set_mleader_styles(table, id, 2);
        assert!(matches!(
            doc.validate(),
            Err(DocumentError::BadMLeaderStyle(_))
        ));
    }

    #[test]
    fn reserving_only_raises_the_counter() {
        let mut doc = Document::default();
        doc.reserve_mleader_style_id(7);
        doc.reserve_mleader_style_id(3);
        assert_eq!(doc.next_mleader_style_id(), 7);
    }
}
