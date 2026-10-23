//! XLINE and RAY records: a base point (10/20) and a direction (11/21), which
//! DXF gives as a unit vector. A direction that is not a unit vector is made
//! one; none at all is a damaged record.

use od_core::{infinite, Entity};

use super::{Importer, Record};

impl Importer {
    pub(super) fn infinite_line(&mut self, record: &Record) {
        let found = (
            Self::point(record, 10, 20),
            Self::point(record, 11, 21).and_then(infinite::unit),
        );
        let (Some(origin), Some(direction)) = found else {
            self.damaged += 1;
            return;
        };
        let entity = if record.name == "RAY" {
            Entity::Ray { origin, direction }
        } else {
            Entity::Xline { origin, direction }
        };
        self.add(record, entity);
    }
}
