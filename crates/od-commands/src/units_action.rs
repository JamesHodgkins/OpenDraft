//! Undoable change of a drawing's unit settings.

use od_core::{Document, Units};

use crate::{Action, Changes};

/// Replace the drawing's units (the UNITS command and its variables).
pub struct SetUnits {
    /// Applying and reverting both swap, so each leaves the other state ready.
    units: Units,
}

impl SetUnits {
    pub fn new(units: Units) -> Self {
        Self {
            units: units.sanitized(),
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.units = doc.set_units(self.units);
    }
}

impl Action for SetUnits {
    fn name(&self) -> String {
        "Units".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::units()
    }
}

#[cfg(test)]
mod tests {
    use od_core::LengthFormat;

    use super::*;

    #[test]
    fn units_change_and_come_back() {
        let mut doc = Document::default();
        let before = *doc.units();
        let mut action = SetUnits::new(Units {
            length: LengthFormat::Architectural,
            length_precision: 99,
            ..before
        });
        action.apply(&mut doc);
        assert_eq!(doc.units().length, LengthFormat::Architectural);
        assert_eq!(doc.units().length_precision, 8, "kept in range");
        action.revert(&mut doc);
        assert_eq!(*doc.units(), before);
        action.apply(&mut doc);
        assert_eq!(doc.units().length, LengthFormat::Architectural);
        assert_eq!(action.changes(), Changes::units());
    }
}
