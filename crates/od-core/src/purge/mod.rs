//! PURGE: what in a drawing nothing uses any more, and why the rest cannot
//! go. Pure; `purgeable` lists what can be deleted, `blocked` what cannot
//! and why, and the command removes things through the document's own
//! actions.

mod scan;
#[cfg(test)]
mod tests;

pub use scan::{blocked, purgeable, unnamed, unused_blocks, users_of, Unnamed};

use crate::text_style::TextStyleId;
use crate::{BlockId, EntityId, LayerId};

/// A kind of purgeable thing, in the order the dialog lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Blocks,
    DimStyles,
    Layers,
    Linetypes,
    MLeaderStyles,
    TextStyles,
    ZeroLength,
    EmptyText,
}

impl Category {
    /// The kinds that are named records of the drawing.
    pub const NAMED: [Category; 6] = [
        Category::Blocks,
        Category::DimStyles,
        Category::Layers,
        Category::Linetypes,
        Category::MLeaderStyles,
        Category::TextStyles,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Blocks => "Blocks",
            Category::DimStyles => "Dimension styles",
            Category::Layers => "Layers",
            Category::Linetypes => "Linetypes",
            Category::MLeaderStyles => "Multileader styles",
            Category::TextStyles => "Text styles",
            Category::ZeroLength => "Zero-length geometry",
            Category::EmptyText => "Empty text objects",
        }
    }
}

/// One thing that can go. Styles and linetypes are named by their name, as
/// the drawing refers to them.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Block(BlockId),
    DimStyle(String),
    Layer(LayerId),
    Linetype(String),
    MLeaderStyle(String),
    TextStyle(TextStyleId),
    ZeroLength(Vec<EntityId>),
    EmptyText(Vec<EntityId>),
}

impl Item {
    pub fn category(&self) -> Category {
        match self {
            Item::Block(_) => Category::Blocks,
            Item::DimStyle(_) => Category::DimStyles,
            Item::Layer(_) => Category::Layers,
            Item::Linetype(_) => Category::Linetypes,
            Item::MLeaderStyle(_) => Category::MLeaderStyles,
            Item::TextStyle(_) => Category::TextStyles,
            Item::ZeroLength(_) => Category::ZeroLength,
            Item::EmptyText(_) => Category::EmptyText,
        }
    }
}

/// An item with the name to show for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Purgeable {
    pub item: Item,
    pub name: String,
}

/// A named item that cannot be purged, and why (every reason that applies).
#[derive(Clone, Debug, PartialEq)]
pub struct Blocked {
    pub item: Item,
    pub name: String,
    pub reasons: Vec<String>,
}
