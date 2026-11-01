//! What kind of object an [`Entity`](crate::Entity) is, as a value to compare
//! and match on rather than a word.

/// One variant per [`Entity`](crate::Entity) variant. Code that tests or
/// groups objects by kind uses this, and the names people and the wire see are
/// separate (`display_name`, `title`, `wire_name`), so rewording a label can
/// never change what a filter selects or what an outside caller is sent.
///
/// The variants run in the alphabetical order of their display names, so the
/// derived ordering lists kinds as the dialogs do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Arc,
    AttributeDefinition,
    Insert,
    Circle,
    Xline,
    Dimension,
    Ellipse,
    Hatch,
    Leader,
    Line,
    MText,
    MLeader,
    Point,
    Polyline,
    Ray,
    Rectangle,
    Spline,
}

impl EntityKind {
    /// Every kind, in order.
    pub const ALL: [EntityKind; 17] = [
        EntityKind::Arc,
        EntityKind::AttributeDefinition,
        EntityKind::Insert,
        EntityKind::Circle,
        EntityKind::Xline,
        EntityKind::Dimension,
        EntityKind::Ellipse,
        EntityKind::Hatch,
        EntityKind::Leader,
        EntityKind::Line,
        EntityKind::MText,
        EntityKind::MLeader,
        EntityKind::Point,
        EntityKind::Polyline,
        EntityKind::Ray,
        EntityKind::Rectangle,
        EntityKind::Spline,
    ];

    /// The kind in running text: "Added line", "Add block reference".
    pub fn display_name(self) -> &'static str {
        match self {
            EntityKind::Arc => "arc",
            EntityKind::AttributeDefinition => "attribute definition",
            EntityKind::Insert => "block reference",
            EntityKind::Circle => "circle",
            EntityKind::Xline => "construction line",
            EntityKind::Dimension => "dimension",
            EntityKind::Ellipse => "ellipse",
            EntityKind::Hatch => "hatch",
            EntityKind::Leader => "leader",
            EntityKind::Line => "line",
            EntityKind::MText => "mtext",
            EntityKind::MLeader => "multileader",
            EntityKind::Point => "point",
            EntityKind::Polyline => "polyline",
            EntityKind::Ray => "ray",
            EntityKind::Rectangle => "rectangle",
            EntityKind::Spline => "spline",
        }
    }

    /// The kind as a heading or list entry: "Block reference".
    pub fn title(self) -> &'static str {
        match self {
            EntityKind::Arc => "Arc",
            EntityKind::AttributeDefinition => "Attribute definition",
            EntityKind::Insert => "Block reference",
            EntityKind::Circle => "Circle",
            EntityKind::Xline => "Construction line",
            EntityKind::Dimension => "Dimension",
            EntityKind::Ellipse => "Ellipse",
            EntityKind::Hatch => "Hatch",
            EntityKind::Leader => "Leader",
            EntityKind::Line => "Line",
            EntityKind::MText => "Mtext",
            EntityKind::MLeader => "Multileader",
            EntityKind::Point => "Point",
            EntityKind::Polyline => "Polyline",
            EntityKind::Ray => "Ray",
            EntityKind::Rectangle => "Rectangle",
            EntityKind::Spline => "Spline",
        }
    }

    /// The token outside callers use (the MCP tools and their JSON). It is
    /// part of that contract, so it only ever changes with a version bump.
    pub fn wire_name(self) -> &'static str {
        match self {
            EntityKind::Arc => "arc",
            EntityKind::AttributeDefinition => "attribute_definition",
            EntityKind::Insert => "block_reference",
            EntityKind::Circle => "circle",
            EntityKind::Xline => "xline",
            EntityKind::Dimension => "dimension",
            EntityKind::Ellipse => "ellipse",
            EntityKind::Hatch => "hatch",
            EntityKind::Leader => "leader",
            EntityKind::Line => "line",
            EntityKind::MText => "mtext",
            EntityKind::MLeader => "multileader",
            EntityKind::Point => "point",
            EntityKind::Polyline => "polyline",
            EntityKind::Ray => "ray",
            EntityKind::Rectangle => "rectangle",
            EntityKind::Spline => "spline",
        }
    }

    /// The kind a wire name stands for, ignoring case; `None` for a word that
    /// is not one.
    pub fn from_wire(name: &str) -> Option<EntityKind> {
        EntityKind::ALL
            .into_iter()
            .find(|k| k.wire_name().eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_kind_once_in_display_order() {
        let names: Vec<_> = EntityKind::ALL.iter().map(|k| k.display_name()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        let mut by_ord = EntityKind::ALL;
        by_ord.sort();
        assert_eq!(by_ord, EntityKind::ALL);
    }

    #[test]
    fn titles_are_the_display_names_capitalised() {
        for kind in EntityKind::ALL {
            let mut chars = kind.display_name().chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase());
            let title: String = first.into_iter().chain(chars).collect();
            assert_eq!(kind.title(), title);
        }
    }

    #[test]
    fn wire_names_round_trip_and_are_single_tokens() {
        for kind in EntityKind::ALL {
            let wire = kind.wire_name();
            assert!(!wire.contains(' '), "{wire}");
            assert_eq!(wire, wire.to_lowercase());
            assert_eq!(EntityKind::from_wire(wire), Some(kind));
        }
        assert_eq!(
            EntityKind::from_wire("Block_Reference"),
            Some(EntityKind::Insert)
        );
        assert_eq!(EntityKind::from_wire("block reference"), None);
        assert_eq!(EntityKind::from_wire("sphere"), None);
    }

    #[test]
    fn the_wire_names_are_the_ones_callers_already_use() {
        let wire: Vec<_> = EntityKind::ALL.iter().map(|k| k.wire_name()).collect();
        assert_eq!(
            wire,
            [
                "arc",
                "attribute_definition",
                "block_reference",
                "circle",
                "xline",
                "dimension",
                "ellipse",
                "hatch",
                "leader",
                "line",
                "mtext",
                "multileader",
                "point",
                "polyline",
                "ray",
                "rectangle",
                "spline"
            ]
        );
    }
}
