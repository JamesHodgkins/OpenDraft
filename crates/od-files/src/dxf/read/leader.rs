//! LEADER entities (the legacy leader), read from AutoCAD's layout
//! (`private_material/LEADERS1.dxf`): the vertices, the arrow and hookline
//! flags, and the handle (340) of the MTEXT it annotates, which comes later
//! in the file and so is looked up once everything is read.
//!
//! A spline path is skipped by reason; a leader not drawn in the XY plane
//! too. The arrowhead's size is the dimension style's `DIMASZ` times its
//! `DIMSCALE`, read from the DIMSTYLE table.

use std::collections::HashMap;

use od_core::leader::Leader;
use od_core::{Entity, EntityId, Point};

use super::{decode, Importer, Record};

/// The arrow size a leader gets when its style is not in the file.
const FALLBACK_ARROW: f64 = 0.18;

/// What the file's leaders need besides their own records.
#[derive(Default)]
pub(super) struct Leaders {
    /// `DIMASZ` times `DIMSCALE`, by lower-cased dimension style name.
    arrows: HashMap<String, f64>,
    /// Each leader read and the handle of the text it names.
    pending: Vec<(EntityId, String)>,
}

impl Importer {
    /// Everything that is read before the entities that use it: the text
    /// styles, the multileader styles and the dimension styles' arrows.
    pub(super) fn scan_styles(&mut self, records: &[Record]) {
        self.scan_text_styles(records);
        self.scan_mleader_styles(records);
        for record in records.iter().filter(|r| r.name == "DIMSTYLE") {
            let Some(name) = record.text(2) else {
                continue;
            };
            let scale = record.number(40).filter(|s| *s > 0.0).unwrap_or(1.0);
            let size = record.number(41).unwrap_or(FALLBACK_ARROW);
            self.leaders
                .arrows
                .insert(decode(name).to_lowercase(), size * scale);
        }
    }

    /// A LEADER entity: into the drawing, or counted by why not.
    pub(super) fn leader(&mut self, record: &Record) {
        match self.read_leader(record) {
            Ok((leader, handle)) if leader.validate().is_ok() => {
                self.add(record, Entity::Leader(Box::new(leader)));
                // A leader inside a block definition keeps no text: the
                // text is its own object there.
                if let (Some(handle), false) = (handle, self.reading_block()) {
                    if let Some(object) = self.document.objects().last() {
                        self.leaders.pending.push((object.id, handle));
                    }
                }
            }
            Ok(_) => self.damaged += 1,
            Err(why) => self.skip(&format!("LEADER ({why})")),
        }
    }

    fn read_leader(&self, record: &Record) -> Result<(Leader, Option<String>), &'static str> {
        if record.int(72).unwrap_or(0) != 0 {
            return Err("spline path");
        }
        let normal = (
            record.number(210).unwrap_or(0.0),
            record.number(220).unwrap_or(0.0),
            record.number(230).unwrap_or(1.0),
        );
        if normal != (0.0, 0.0, 1.0) {
            return Err("not drawn in the XY plane");
        }
        let mut vertices = Vec::new();
        let mut x = None;
        for g in &record.groups {
            let num = g.value.parse::<f64>().ok().filter(|v| v.is_finite());
            match g.code {
                10 => x = num,
                20 => vertices.push(Point::new(
                    x.take().ok_or("damaged")?,
                    num.ok_or("damaged")?,
                )),
                _ => {}
            }
        }
        let name = decode(record.text(3).unwrap_or("Standard"));
        let size = self
            .leaders
            .arrows
            .get(&name.to_lowercase())
            .copied()
            .unwrap_or(FALLBACK_ARROW);
        let mut leader = Leader::new(vertices, size);
        leader.arrow = record.int(71).unwrap_or(1) != 0;
        leader.hookline = record.int(75).unwrap_or(0) != 0;
        leader.hook_same_direction = record.int(74).unwrap_or(0) != 0;
        leader.dim_style_name = name;
        leader.annotation_size = (
            record.number(40).unwrap_or(0.0).max(0.0),
            record.number(41).unwrap_or(0.0).max(0.0),
        );
        Ok((leader, record.text(340).map(str::to_owned)))
    }

    /// Give each leader the text its handle names, now every object is read.
    /// A handle that names nothing, or something that is not text, leaves the
    /// leader without an annotation.
    pub(super) fn resolve_leaders(&mut self) {
        for (id, handle) in std::mem::take(&mut self.leaders.pending) {
            let Some(text) = self.handles.get(&handle.to_uppercase()).copied() else {
                continue;
            };
            if !matches!(self.document.get(text), Some(Entity::MText(_))) {
                continue;
            }
            if let Some(Entity::Leader(l)) = self.document.get(id) {
                let mut with_text = (**l).clone();
                with_text.annotation = Some(text);
                self.document
                    .replace(id, Entity::Leader(Box::new(with_text)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::read::read as read_dxf;

    fn leaders(doc: &od_core::Document) -> Vec<(EntityId, Leader)> {
        doc.objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Leader(l) => Some((o.id, (**l).clone())),
                _ => None,
            })
            .collect()
    }

    fn file(leader: &str) -> Vec<u8> {
        format!("0\nSECTION\n2\nENTITIES\n0\nLEADER\n{leader}0\nENDSEC\n0\nEOF\n").into_bytes()
    }

    #[test]
    fn a_leader_with_no_style_gets_the_fallback_arrow_and_a_missing_text_is_none() {
        let loaded = read_dxf(&file(
            "3\nNope\n76\n2\n10\n0\n20\n0\n30\n0\n10\n5\n20\n5\n30\n0\n340\nFFFF\n",
        ))
        .unwrap();
        let found = leaders(&loaded.document);
        assert_eq!(found[0].1.arrow_size, FALLBACK_ARROW);
        assert_eq!(found[0].1.annotation, None);
    }

    #[test]
    fn what_cannot_come_in_is_said_by_reason() {
        for (groups, reason) in [
            ("72\n1\n10\n0\n20\n0\n10\n5\n20\n5\n", "spline path"),
            (
                "230\n-1\n10\n0\n20\n0\n10\n5\n20\n5\n",
                "not drawn in the XY plane",
            ),
        ] {
            let loaded = read_dxf(&file(groups)).unwrap();
            assert!(leaders(&loaded.document).is_empty());
            assert!(loaded.warnings.join(" ").contains(reason), "{reason}");
        }
        let loaded = read_dxf(&file("10\n0\n20\n0\n")).unwrap();
        assert!(
            leaders(&loaded.document).is_empty(),
            "one point is no leader"
        );
        assert!(loaded.warnings.join(" ").contains("damaged"));
    }
}
