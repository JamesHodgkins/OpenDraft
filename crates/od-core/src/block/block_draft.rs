//! What the Block Definition dialog edits: a plain draft of its values,
//! checked in one place, so the dialog only draws.

use crate::block::block_name_problem;
use crate::{EntityId, InsertUnits, Point};

/// What becomes of the objects the block is made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectsAction {
    /// They stay as they are.
    Retain,
    /// They are replaced by a reference to the new block.
    Convert,
    /// They are deleted.
    Delete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockDraft {
    pub name: String,
    /// The base point is picked in the drawing after OK, not typed.
    pub base_on_screen: bool,
    pub x: f64,
    pub y: f64,
    /// The objects are selected in the drawing after OK.
    pub objects_on_screen: bool,
    pub objects: Vec<EntityId>,
    pub action: ObjectsAction,
    pub description: String,
    /// What one unit of the block's coordinates is.
    pub unit: InsertUnits,
    /// Open the new block in the Block Editor once it is made.
    pub open_in_editor: bool,
}

impl Default for BlockDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_on_screen: false,
            x: 0.0,
            y: 0.0,
            objects_on_screen: false,
            objects: Vec::new(),
            action: ObjectsAction::Convert,
            description: String::new(),
            unit: InsertUnits::UNITLESS,
            open_in_editor: false,
        }
    }
}

impl BlockDraft {
    /// The dialog as it opens, on the objects already selected.
    pub fn new(selected: &[EntityId]) -> Self {
        Self {
            objects: selected.to_vec(),
            ..Self::default()
        }
    }

    /// The base point as typed.
    pub fn base(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Why the dialog cannot be accepted, if it cannot.
    pub fn problem(&self) -> Option<String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Some("Enter a name for the block.".to_owned());
        }
        if let Some(why) = block_name_problem(name) {
            return Some(why.to_owned());
        }
        if !(self.base_on_screen || (self.x.is_finite() && self.y.is_finite())) {
            return Some("The base point is not a number.".to_owned());
        }
        if !self.objects_on_screen && self.objects.is_empty() {
            return Some("No objects selected.".to_owned());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_on_the_selection_and_converts_by_default() {
        let d = BlockDraft::new(&[EntityId(3), EntityId(4)]);
        assert_eq!(d.objects, [EntityId(3), EntityId(4)]);
        assert_eq!(d.action, ObjectsAction::Convert);
        assert_eq!(d.base(), Point::new(0.0, 0.0));
    }

    #[test]
    fn it_needs_a_valid_name_and_objects_picked_or_to_be_picked() {
        let mut d = BlockDraft::default();
        assert!(d.problem().unwrap().contains("name"));
        d.name = "a|b".into();
        assert!(d.problem().is_some(), "a character AutoCAD refuses");
        d.name = "Door".into();
        assert_eq!(d.problem().as_deref(), Some("No objects selected."));
        d.objects_on_screen = true;
        assert_eq!(d.problem(), None, "they are chosen after OK");
        d.objects_on_screen = false;
        d.objects.push(EntityId(1));
        assert_eq!(d.problem(), None);
        d.x = f64::NAN;
        assert!(d.problem().is_some());
        d.base_on_screen = true;
        assert_eq!(d.problem(), None, "the point is picked after OK");
    }
}
