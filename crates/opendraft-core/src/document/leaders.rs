//! Checks on the legacy leaders of a drawing.

use super::Document;
use crate::validate::DocumentError;
use crate::{Entity, EntityId};

impl Document {
    /// Every leader could exist. The text one annotates may be gone: the
    /// reference then names nothing, which is allowed.
    pub(crate) fn validate_leaders(&self) -> Result<(), DocumentError> {
        for object in self.objects() {
            if let Entity::Leader(l) = &object.entity {
                if let Err(why) = l.validate() {
                    return Err(DocumentError::BadLeader(object.id, why.to_string()));
                }
            }
        }
        Ok(())
    }

    /// The text the leader `id` annotates, when both are there.
    pub fn leader_annotation(&self, id: EntityId) -> Option<&crate::MText> {
        let Some(Entity::Leader(l)) = self.get(id) else {
            return None;
        };
        match self.get(l.annotation?) {
            Some(Entity::MText(text)) => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leader::Leader;
    use crate::{MText, Point, TextStyleId};

    #[test]
    fn the_text_is_found_while_it_is_there_and_a_gone_text_is_not_an_error() {
        let mut doc = Document::default();
        let text = doc.add(Entity::MText(Box::new(MText::new(
            Point::new(5.0, 5.0),
            2.5,
            "Note",
            TextStyleId::STANDARD,
        ))));
        let mut leader = Leader::new(vec![Point::new(0.0, 0.0), Point::new(4.0, 4.0)], 1.0);
        leader.annotation = Some(text);
        let id = doc.add(Entity::Leader(Box::new(leader)));
        assert_eq!(doc.leader_annotation(id).unwrap().contents, "Note");
        assert!(doc.remove(text).is_some());
        assert!(doc.leader_annotation(id).is_none());
        assert_eq!(doc.validate(), Ok(()));
    }

    #[test]
    fn a_leader_that_could_not_exist_is_reported() {
        let mut doc = Document::default();
        let bad = Leader::new(vec![Point::new(0.0, 0.0)], 1.0);
        let id = doc.add(Entity::Leader(Box::new(bad)));
        assert!(matches!(
            doc.validate(),
            Err(DocumentError::BadLeader(bad, _)) if bad == id
        ));
    }
}
