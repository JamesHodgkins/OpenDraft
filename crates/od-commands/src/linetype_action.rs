//! Undoable change of the drawing's linetypes: the table and `LTSCALE`.

use od_core::{Document, LinetypeTable};

use crate::{Action, Changes};

/// Replace the linetype table and `LTSCALE` (the Linetype Manager, and the
/// `LTSCALE` variable).
pub struct SetLinetypes {
    /// Applying and reverting both swap, so each leaves the other state ready.
    table: LinetypeTable,
    scale: f64,
}

impl SetLinetypes {
    pub fn new(table: LinetypeTable, scale: f64) -> Self {
        Self { table, scale }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.scale = doc.set_linetype_scale(self.scale);
        self.table = doc.set_linetypes(self.table.clone());
    }
}

impl Action for SetLinetypes {
    fn name(&self) -> String {
        "Linetypes".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::linetypes()
    }
}

#[cfg(test)]
mod tests {
    use od_core::StrokeElement;

    use super::*;

    #[test]
    fn the_table_and_scale_change_and_come_back() {
        let mut doc = Document::default();
        let mut table = doc.linetypes().clone();
        table
            .add(
                "Mine",
                "",
                vec![StrokeElement::Dash(2.0), StrokeElement::Gap(1.0)],
            )
            .unwrap();
        let mut action = SetLinetypes::new(table, 2.5);
        action.apply(&mut doc);
        assert_eq!(doc.linetype_scale(), 2.5);
        assert!(doc.linetypes().named("Mine").is_some());
        assert_eq!(action.changes(), Changes::linetypes());
        action.revert(&mut doc);
        assert_eq!(doc.linetype_scale(), 1.0);
        assert!(doc.linetypes().is_stock());
        action.apply(&mut doc);
        assert!(doc.linetypes().named("Mine").is_some());
    }
}
