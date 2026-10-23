//! Undoable change of the multileader-style table: the styles, the current
//! style, the id counter and the multileaders made in a style that changed.

use od_core::{Document, Entity, EntityId, MLeaderStyle, MLeaderStyleId};

use crate::{Action, Changes};

/// Replace the multileader-style table and restyle some multileaders with it
/// (the Multileader Style Manager).
pub struct SetMLeaderStyles {
    /// Applying and reverting both swap, so each leaves the other state ready.
    styles: Vec<MLeaderStyle>,
    current: MLeaderStyleId,
    next_id: u64,
    /// The multileaders to change and what to change them to; after `apply`
    /// they hold what was there before.
    edits: Vec<(EntityId, Entity)>,
    name: String,
}

impl SetMLeaderStyles {
    pub fn new(
        name: impl Into<String>,
        styles: Vec<MLeaderStyle>,
        current: MLeaderStyleId,
        next_id: u64,
        edits: Vec<(EntityId, Entity)>,
    ) -> Self {
        Self {
            styles,
            current,
            next_id,
            edits,
            name: name.into(),
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        let (styles, current, next_id) =
            doc.set_mleader_styles(std::mem::take(&mut self.styles), self.current, self.next_id);
        self.styles = styles;
        self.current = current;
        self.next_id = next_id;
        for (id, entity) in &mut self.edits {
            if let Some(old) = doc.replace(*id, entity.clone()) {
                *entity = old;
            }
        }
    }
}

impl Action for SetMLeaderStyles {
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
            mleader_styles: true,
            ..Changes::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_changes_and_comes_back() {
        let mut doc = Document::default();
        let notes = MLeaderStyleId(doc.next_mleader_style_id());
        let mut table = doc.mleader_styles().to_vec();
        table.push(MLeaderStyle::new(notes, "Notes"));
        let mut action = SetMLeaderStyles::new("New", table, notes, notes.0 + 1, Vec::new());
        action.apply(&mut doc);
        assert_eq!(doc.mleader_styles().len(), 2);
        assert_eq!(doc.current_mleader_style(), notes);
        assert_eq!(doc.validate(), Ok(()));
        action.revert(&mut doc);
        assert_eq!(doc.mleader_styles().len(), 1);
        assert_eq!(doc.current_mleader_style(), MLeaderStyleId::STANDARD);
        assert!(action.changes().mleader_styles);
        assert!(!action.changes().is_empty());
    }
}
