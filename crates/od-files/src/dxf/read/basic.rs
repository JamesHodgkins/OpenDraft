//! LINE, CIRCLE, ARC and POINT records.

use od_core::{arc, Entity};

use super::{Importer, Record};

impl Importer {
    pub(super) fn basic(&mut self, record: &Record) {
        match record.name.as_str() {
            "LINE" => self.line(record),
            "CIRCLE" => self.circle(record),
            "ARC" => self.arc(record),
            _ => self.point_entity(record),
        }
    }

    fn point_entity(&mut self, record: &Record) {
        match Self::point(record, 10, 20) {
            Some(at) => self.add(record, Entity::Point { at }),
            None => self.damaged += 1,
        }
    }

    fn line(&mut self, record: &Record) {
        match (Self::point(record, 10, 20), Self::point(record, 11, 21)) {
            (Some(start), Some(end)) => self.add(record, Entity::Line { start, end }),
            _ => self.damaged += 1,
        }
    }

    fn circle(&mut self, record: &Record) {
        match (Self::point(record, 10, 20), record.number(40)) {
            (Some(center), Some(radius)) if radius > 0.0 => {
                self.add(record, Entity::Circle { center, radius });
            }
            _ => self.damaged += 1,
        }
    }

    fn arc(&mut self, record: &Record) {
        match (
            Self::point(record, 10, 20),
            record.number(40),
            record.number(50),
            record.number(51),
        ) {
            (Some(center), Some(radius), Some(start), Some(end)) if radius > 0.0 => {
                self.add(
                    record,
                    Entity::Arc {
                        center,
                        radius,
                        start: arc::normalize(start.to_radians()),
                        end: arc::normalize(end.to_radians()),
                    },
                );
            }
            _ => self.damaged += 1,
        }
    }
}
