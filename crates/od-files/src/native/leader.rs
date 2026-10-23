//! The native format's description of a legacy leader.

use od_core::leader::Leader;
use od_core::EntityId;
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub(super) struct LeaderDto {
    vertices: Vec<PointDto>,
    #[serde(default = "yes")]
    arrow: bool,
    arrow_size: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    hookline: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    hook_same_direction: bool,
    dim_style: String,
    /// The text it annotates; may name an object that is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annotation: Option<u64>,
    #[serde(default)]
    annotation_size: [f64; 2],
}

impl LeaderDto {
    pub(super) fn from_leader(l: &Leader) -> Result<Self, FileError> {
        if let Err(why) = l.validate() {
            return invalid(format!("a leader is not valid: {why}"));
        }
        Ok(Self {
            vertices: l.vertices.iter().map(|p| point(*p)).collect(),
            arrow: l.arrow,
            arrow_size: l.arrow_size,
            hookline: l.hookline,
            hook_same_direction: l.hook_same_direction,
            dim_style: l.dim_style_name.clone(),
            annotation: l.annotation.map(|id| id.0),
            annotation_size: [l.annotation_size.0, l.annotation_size.1],
        })
    }

    pub(super) fn to_leader(&self) -> Result<Leader, FileError> {
        let vertices = self
            .vertices
            .iter()
            .map(|p| from_point(*p))
            .collect::<Result<Vec<_>, _>>()?;
        let l = Leader {
            vertices,
            arrow: self.arrow,
            arrow_size: self.arrow_size,
            hookline: self.hookline,
            hook_same_direction: self.hook_same_direction,
            dim_style_name: self.dim_style.clone(),
            annotation: self.annotation.map(EntityId),
            annotation_size: (self.annotation_size[0], self.annotation_size[1]),
        };
        match l.validate() {
            Ok(()) => Ok(l),
            Err(why) => invalid(format!("a leader is not valid: {why}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Document, Entity, Point};

    use crate::native::tests::{read, roundtrip, MINIMAL};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn leaders_survive_a_round_trip_exactly() {
        let mut document = Document::default();
        let text = document.add(Entity::MText(Box::new(od_core::MText::new(
            p(15.0, 10.0),
            2.5,
            "Note",
            od_core::TextStyleId::STANDARD,
        ))));
        let mut with_text = Leader::new(vec![p(0.0, 0.0), p(10.0, 10.0), p(14.0, 10.0)], 2.5);
        with_text.hookline = true;
        with_text.dim_style_name = "ISO-25".into();
        with_text.annotation = Some(text);
        with_text.annotation_size = (2.5, 8.0);
        let mut bare = Leader::new(vec![p(1.0, 1.0), p(9.0, 4.0)], 1.0);
        bare.arrow = false;
        // A reference to text that is gone is kept as it is.
        bare.annotation = Some(EntityId(999));
        document.add(Entity::Leader(Box::new(with_text)));
        document.add(Entity::Leader(Box::new(bare)));
        assert_eq!(roundtrip(&document), document);
    }

    fn file(entity: &str) -> String {
        MINIMAL.replace(
            "\"objects\": []",
            &format!("\"objects\": [{{\"id\": 0, \"layer\": 0, \"entity\": {entity}}}]"),
        )
    }

    #[test]
    fn a_bad_leader_is_refused_not_repaired() {
        let good = r#"{"type": "leader", "vertices": [[0.0, 0.0], [5.0, 5.0]],
            "arrow_size": 2.5, "dim_style": "Standard"}"#;
        assert!(read(&file(good)).is_ok());
        for bad in [
            good.replace("[[0.0, 0.0], [5.0, 5.0]]", "[[0.0, 0.0]]"),
            good.replace("[5.0, 5.0]", "[0.0, 0.0]"),
            good.replace("2.5", "-2.5"),
        ] {
            assert!(read(&file(&bad)).is_err(), "{bad}");
        }
    }
}
