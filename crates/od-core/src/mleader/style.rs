//! Multileader styles: the named record a multileader takes its leader line,
//! arrow, structure and text from. The table itself lives on the `Document`.

use crate::dimension::Arrow;
use crate::layer::{Attr, Color, Lineweight};
use crate::linetype::LinetypeId;
use crate::text_style::{TextStyleId, MAX_NAME_BYTES};

/// Permanent identity of a multileader style; never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MLeaderStyleId(pub u64);

impl MLeaderStyleId {
    /// The style every drawing starts with.
    pub const STANDARD: MLeaderStyleId = MLeaderStyleId(0);
}

/// Most leader points a style may allow.
pub const MAX_POINTS: u32 = 4_096;
/// Longest default text a style keeps.
pub const MAX_DEFAULT_TEXT_BYTES: usize = 4_096;

/// How the leader line runs (the Leader Format tab's Type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MLeaderLineType {
    /// No leader line: the content stands alone.
    None,
    #[default]
    Straight,
    Spline,
}

impl MLeaderLineType {
    pub const ALL: [MLeaderLineType; 3] = [Self::Straight, Self::Spline, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Straight => "Straight",
            Self::Spline => "Spline",
        }
    }
}

/// What a multileader points at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MLeaderContentType {
    /// Leader lines only.
    None,
    #[default]
    MText,
}

impl MLeaderContentType {
    pub const ALL: [MLeaderContentType; 2] = [Self::MText, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::MText => "Mtext",
        }
    }
}

/// How the text is turned (the Content tab's Text angle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAngle {
    /// Always horizontal, whatever way the leader runs.
    #[default]
    KeepHorizontal,
    /// Along the last leader segment, turned over when it would read upside down.
    RightReading,
    /// Along the last leader segment as drawn.
    AsInserted,
}

impl TextAngle {
    pub const ALL: [TextAngle; 3] = [Self::KeepHorizontal, Self::RightReading, Self::AsInserted];

    pub fn label(self) -> &'static str {
        match self {
            Self::KeepHorizontal => "Keep horizontal",
            Self::RightReading => "Right-reading",
            Self::AsInserted => "As inserted",
        }
    }
}

/// Where on the text the landing joins it, for text to the left or right of
/// the leader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeaderAttachment {
    TopOfTopLine,
    #[default]
    MiddleOfTopLine,
    MiddleOfText,
    MiddleOfBottomLine,
    BottomOfBottomLine,
    BottomLine,
    UnderlineBottomLine,
    UnderlineTopLine,
    UnderlineAllText,
}

impl LeaderAttachment {
    pub const ALL: [LeaderAttachment; 9] = [
        Self::TopOfTopLine,
        Self::MiddleOfTopLine,
        Self::MiddleOfText,
        Self::MiddleOfBottomLine,
        Self::BottomOfBottomLine,
        Self::BottomLine,
        Self::UnderlineBottomLine,
        Self::UnderlineTopLine,
        Self::UnderlineAllText,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::TopOfTopLine => "Top of top line",
            Self::MiddleOfTopLine => "Middle of top line",
            Self::MiddleOfText => "Middle of text",
            Self::MiddleOfBottomLine => "Middle of bottom line",
            Self::BottomOfBottomLine => "Bottom of bottom line",
            Self::BottomLine => "Bottom line",
            Self::UnderlineBottomLine => "Underline bottom line",
            Self::UnderlineTopLine => "Underline top line",
            Self::UnderlineAllText => "Underline all text",
        }
    }
}

/// The Content tab: the text a multileader carries.
#[derive(Clone, Debug, PartialEq)]
pub struct MLeaderStyleContent {
    pub kind: MLeaderContentType,
    pub default_text: String,
    pub text_style: TextStyleId,
    pub text_angle: TextAngle,
    pub text_color: Attr<Color>,
    pub text_height: f64,
    pub always_left_justify: bool,
    pub frame: bool,
    pub left_attachment: LeaderAttachment,
    pub right_attachment: LeaderAttachment,
    /// The space between the landing and the text.
    pub landing_gap: f64,
    /// Run the leader on to the text's far edge (`Extend leader to text`).
    pub extend_to_text: bool,
}

impl Default for MLeaderStyleContent {
    fn default() -> Self {
        Self {
            kind: MLeaderContentType::MText,
            default_text: String::new(),
            text_style: TextStyleId::STANDARD,
            text_angle: TextAngle::KeepHorizontal,
            text_color: Attr::ByLayer,
            text_height: 2.5,
            always_left_justify: false,
            frame: false,
            left_attachment: LeaderAttachment::MiddleOfTopLine,
            right_attachment: LeaderAttachment::MiddleOfTopLine,
            landing_gap: 2.0,
            extend_to_text: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MLeaderStyle {
    pub id: MLeaderStyleId,
    pub name: String,
    pub description: String,
    // Leader Format
    pub leader_type: MLeaderLineType,
    pub leader_color: Attr<Color>,
    pub leader_linetype: Attr<LinetypeId>,
    pub leader_lineweight: Attr<Lineweight>,
    pub arrow: Arrow,
    pub arrow_size: f64,
    /// The gap a dimension break leaves in a leader line.
    pub break_gap: f64,
    // Leader Structure
    /// Most points a leader may have; `None` leaves it free.
    pub max_points: Option<u32>,
    /// Angles (radians) the first and second segments are held to; `None`
    /// leaves them free.
    pub first_angle: Option<f64>,
    pub second_angle: Option<f64>,
    /// Add a horizontal landing after the last point (`Automatically include landing`).
    pub dogleg: bool,
    /// How long that landing is.
    pub dogleg_length: f64,
    /// Multiplier of every size the style sets.
    pub overall_scale: f64,
    pub content: MLeaderStyleContent,
}

impl MLeaderStyle {
    pub fn new(id: MLeaderStyleId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            leader_type: MLeaderLineType::Straight,
            leader_color: Attr::ByLayer,
            leader_linetype: Attr::ByLayer,
            leader_lineweight: Attr::ByLayer,
            arrow: Arrow::Closed,
            arrow_size: 2.5,
            break_gap: 3.75,
            max_points: Some(2),
            first_angle: None,
            second_angle: None,
            dogleg: true,
            dogleg_length: 8.0,
            overall_scale: 1.0,
            content: MLeaderStyleContent::default(),
        }
    }

    pub fn standard() -> Self {
        Self::new(MLeaderStyleId::STANDARD, "Standard")
    }

    /// Whether the style could exist: a bounded name and text, sizes finite
    /// and not negative, a positive scale, and point counts and angles in range.
    pub fn is_valid(&self) -> bool {
        let size = |v: f64| v.is_finite() && v >= 0.0;
        let angle = |a: Option<f64>| a.is_none_or(f64::is_finite);
        let c = &self.content;
        !self.name.trim().is_empty()
            && self.name.len() <= MAX_NAME_BYTES
            && self.description.len() <= MAX_DEFAULT_TEXT_BYTES
            && c.default_text.len() <= MAX_DEFAULT_TEXT_BYTES
            && size(self.arrow_size)
            && size(self.break_gap)
            && size(self.dogleg_length)
            && size(c.landing_gap)
            && c.text_height.is_finite()
            && c.text_height > 0.0
            && self.overall_scale.is_finite()
            && self.overall_scale > 0.0
            && self
                .max_points
                .is_none_or(|n| (2..=MAX_POINTS).contains(&n))
            && angle(self.first_angle)
            && angle(self.second_angle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stock_style_is_valid() {
        let s = MLeaderStyle::standard();
        assert!(s.is_valid());
        assert_eq!(s.id, MLeaderStyleId::STANDARD);
        assert_eq!(s.name, "Standard");
    }

    #[test]
    fn bad_values_make_a_style_invalid() {
        let ok = MLeaderStyle::standard();
        let bad = [
            MLeaderStyle {
                name: "  ".into(),
                ..ok.clone()
            },
            MLeaderStyle {
                arrow_size: f64::NAN,
                ..ok.clone()
            },
            MLeaderStyle {
                dogleg_length: -1.0,
                ..ok.clone()
            },
            MLeaderStyle {
                overall_scale: 0.0,
                ..ok.clone()
            },
            MLeaderStyle {
                max_points: Some(1),
                ..ok.clone()
            },
            MLeaderStyle {
                first_angle: Some(f64::INFINITY),
                ..ok.clone()
            },
            MLeaderStyle {
                content: MLeaderStyleContent {
                    text_height: 0.0,
                    ..ok.content.clone()
                },
                ..ok.clone()
            },
        ];
        for style in bad {
            assert!(!style.is_valid(), "{style:?}");
        }
        let free = MLeaderStyle {
            max_points: None,
            first_angle: Some(0.5),
            ..ok
        };
        assert!(free.is_valid());
    }
}
