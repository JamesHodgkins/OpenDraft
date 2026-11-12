//! Multileaders: the entity ([`MLeader`]), the style table's record
//! ([`MLeaderStyle`]) and the sample the style dialogs preview.

mod entity;
mod picture;
mod place;
mod restyle;
mod sample;
mod style;
mod transform;

pub use entity::{
    MLeader, MLeaderContent, MLeaderError, MLeaderLook, MLeaderText, MAX_LINES, MAX_LINE_POINTS,
};
pub use place::place_text;
pub use sample::sample;
pub use style::{
    LeaderAttachment, MLeaderContentType, MLeaderLineType, MLeaderStyle, MLeaderStyleContent,
    MLeaderStyleId, TextAngle, MAX_DEFAULT_TEXT_BYTES, MAX_POINTS,
};
