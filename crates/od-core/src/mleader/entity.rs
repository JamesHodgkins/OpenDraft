//! The multileader entity: leader lines from arrowheads to a landing, and
//! the text they point at. The points and the text are stored; everything
//! drawn (arrowheads, landing, frame) is worked out from them, so editing a
//! point always redraws the whole thing.
//!
//! Like a dimension, it keeps a copy of what the style said when it was made
//! (`look`, sizes already multiplied by the style's scale) so it can be drawn
//! without the drawing's tables.

use std::fmt;

use super::place::place_text;
use super::style::{MLeaderContentType, MLeaderLineType, MLeaderStyle};
use crate::dimension::Arrow;
use crate::mtext::{MText, MTextError};
use crate::Point;

/// Most leader lines one multileader holds.
pub const MAX_LINES: usize = 1_024;
/// Most points on one leader line.
pub const MAX_LINE_POINTS: usize = 4_096;

/// What the style said when the multileader was made.
#[derive(Clone, Debug, PartialEq)]
pub struct MLeaderLook {
    pub leader_type: MLeaderLineType,
    pub arrow: Arrow,
    pub arrow_size: f64,
    /// Whether a horizontal landing follows the last point.
    pub dogleg: bool,
    pub dogleg_length: f64,
    pub landing_gap: f64,
}

impl MLeaderLook {
    /// The style's values with its overall scale applied to every size.
    pub fn of(style: &MLeaderStyle) -> Self {
        let k = style.overall_scale;
        Self {
            leader_type: style.leader_type,
            arrow: style.arrow,
            arrow_size: style.arrow_size * k,
            dogleg: style.dogleg,
            dogleg_length: style.dogleg_length * k,
            landing_gap: style.content.landing_gap * k,
        }
    }

    fn is_valid(&self) -> bool {
        [self.arrow_size, self.dogleg_length, self.landing_gap]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MLeaderText {
    pub text: MText,
    /// A box round the text.
    pub frame: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MLeaderContent {
    None,
    MText(MLeaderText),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MLeader {
    /// The style it was made in, which a restyle finds it by.
    pub style_name: String,
    pub look: MLeaderLook,
    /// Each leader line's points, arrowhead first. The landing starts at the
    /// last point of the first line.
    pub lines: Vec<Vec<Point>>,
    /// The unit direction of the landing: east for text to the right.
    pub landing_dir: Point,
    pub content: MLeaderContent,
}

/// What is wrong with a multileader.
#[derive(Clone, Debug, PartialEq)]
pub enum MLeaderError {
    NotFinite,
    NoLines,
    TooManyLines,
    BadLine,
    BadDirection,
    BadSize,
    Text(MTextError),
}

impl fmt::Display for MLeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => write!(f, "a number is not finite"),
            Self::NoLines => write!(f, "there is no leader line"),
            Self::TooManyLines => write!(f, "there are too many leader lines"),
            Self::BadLine => write!(
                f,
                "a leader line needs 2 to {MAX_LINE_POINTS} distinct points"
            ),
            Self::BadDirection => write!(f, "the landing has no direction"),
            Self::BadSize => write!(f, "a size is negative"),
            Self::Text(e) => write!(f, "the text is not valid: {e}"),
        }
    }
}

impl std::error::Error for MLeaderError {}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

impl MLeader {
    /// A multileader in `style` with one leader line through `points`
    /// (arrowhead first) and `contents` as its text. The text goes to the
    /// right of the landing when the last point is right of the arrowhead
    /// and to the left otherwise. `None` when there are fewer than two points.
    pub fn build(style: &MLeaderStyle, points: Vec<Point>, contents: &str) -> Option<MLeader> {
        let (first, tail) = (*points.first()?, *points.last()?);
        if points.len() < 2 {
            return None;
        }
        let leader_on_left = tail.x >= first.x;
        let mut mleader = MLeader {
            style_name: style.name.clone(),
            look: MLeaderLook::of(style),
            lines: vec![points],
            landing_dir: Point::new(if leader_on_left { 1.0 } else { -1.0 }, 0.0),
            content: MLeaderContent::None,
        };
        let c = &style.content;
        if c.kind == MLeaderContentType::MText {
            let height = c.text_height * style.overall_scale;
            let attachment = if leader_on_left {
                c.left_attachment
            } else {
                c.right_attachment
            };
            let (kind, at) = place_text(
                attachment,
                leader_on_left,
                mleader.landing_end(),
                mleader.look.landing_gap,
                height,
            );
            let mut text = MText::new(at, height, contents, c.text_style);
            text.attachment = kind;
            mleader.content = MLeaderContent::MText(MLeaderText {
                text,
                frame: c.frame,
            });
        }
        Some(mleader)
    }

    /// Where the last point of the first line is, which the landing leaves.
    pub fn landing_start(&self) -> Option<Point> {
        self.lines.first()?.last().copied()
    }

    /// Where the landing ends (the landing start when it has no landing).
    pub fn landing_end(&self) -> Point {
        let start = self.landing_start().unwrap_or_default();
        let length = if self.look.dogleg {
            self.look.dogleg_length
        } else {
            0.0
        };
        Point::new(
            start.x + self.landing_dir.x * length,
            start.y + self.landing_dir.y * length,
        )
    }

    /// Whether the multileader could exist.
    pub fn validate(&self) -> Result<(), MLeaderError> {
        if self.lines.is_empty() {
            return Err(MLeaderError::NoLines);
        }
        if self.lines.len() > MAX_LINES {
            return Err(MLeaderError::TooManyLines);
        }
        for line in &self.lines {
            if !line.iter().copied().all(finite) {
                return Err(MLeaderError::NotFinite);
            }
            let distinct = line
                .windows(2)
                .all(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y) > 1e-12);
            if line.len() < 2 || line.len() > MAX_LINE_POINTS || !distinct {
                return Err(MLeaderError::BadLine);
            }
        }
        if !finite(self.landing_dir) {
            return Err(MLeaderError::NotFinite);
        }
        if (self.landing_dir.x.hypot(self.landing_dir.y) - 1.0).abs() > 1e-6 {
            return Err(MLeaderError::BadDirection);
        }
        if !self.look.is_valid() {
            return Err(MLeaderError::BadSize);
        }
        if let MLeaderContent::MText(t) = &self.content {
            t.text.validate().map_err(MLeaderError::Text)?;
        }
        Ok(())
    }

    /// The text, when there is any.
    pub fn text(&self) -> Option<&MLeaderText> {
        match &self.content {
            MLeaderContent::MText(t) => Some(t),
            MLeaderContent::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mleader::LeaderAttachment;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn the_text_goes_to_the_side_the_last_point_is_on() {
        let style = MLeaderStyle::standard();
        let right = MLeader::build(&style, vec![p(0.0, 0.0), p(10.0, 10.0)], "Note").unwrap();
        assert_eq!(right.landing_dir, p(1.0, 0.0));
        assert_eq!(right.landing_end(), p(18.0, 10.0));
        let text = &right.text().unwrap().text;
        assert!(text.insertion.x > 18.0, "the text starts past the gap");
        let left = MLeader::build(&style, vec![p(0.0, 0.0), p(-10.0, 10.0)], "Note").unwrap();
        assert_eq!(left.landing_dir, p(-1.0, 0.0));
        assert!(left.text().unwrap().text.insertion.x < -18.0);
        assert_eq!(right.validate(), Ok(()));
        assert_eq!(left.validate(), Ok(()));
    }

    #[test]
    fn no_landing_leaves_the_text_at_the_last_point() {
        let mut style = MLeaderStyle::standard();
        style.dogleg = false;
        let m = MLeader::build(&style, vec![p(0.0, 0.0), p(10.0, 10.0)], "x").unwrap();
        assert_eq!(m.landing_end(), p(10.0, 10.0));
    }

    #[test]
    fn the_style_scale_multiplies_every_size_once() {
        let mut style = MLeaderStyle::standard();
        style.overall_scale = 2.0;
        let m = MLeader::build(&style, vec![p(0.0, 0.0), p(10.0, 10.0)], "x").unwrap();
        assert_eq!(m.look.arrow_size, 5.0);
        assert_eq!(m.look.dogleg_length, 16.0);
        assert_eq!(m.text().unwrap().text.nominal_height, 5.0);
    }

    #[test]
    fn content_none_has_no_text_and_a_single_point_builds_nothing() {
        let mut style = MLeaderStyle::standard();
        style.content.kind = MLeaderContentType::None;
        let m = MLeader::build(&style, vec![p(0.0, 0.0), p(1.0, 1.0)], "x").unwrap();
        assert!(m.text().is_none());
        assert!(MLeader::build(&style, vec![p(0.0, 0.0)], "x").is_none());
        assert!(MLeader::build(&style, Vec::new(), "x").is_none());
    }

    #[test]
    fn bad_multileaders_are_refused() {
        let style = MLeaderStyle::standard();
        let good = MLeader::build(&style, vec![p(0.0, 0.0), p(5.0, 5.0)], "x").unwrap();
        let mut bad = good.clone();
        bad.lines[0][1] = p(f64::NAN, 0.0);
        assert_eq!(bad.validate(), Err(MLeaderError::NotFinite));
        let mut bad = good.clone();
        bad.lines[0][1] = bad.lines[0][0];
        assert_eq!(bad.validate(), Err(MLeaderError::BadLine));
        let mut bad = good.clone();
        bad.lines.clear();
        assert_eq!(bad.validate(), Err(MLeaderError::NoLines));
        let mut bad = good.clone();
        bad.landing_dir = p(0.0, 0.0);
        assert_eq!(bad.validate(), Err(MLeaderError::BadDirection));
        let mut bad = good.clone();
        bad.look.arrow_size = -1.0;
        assert_eq!(bad.validate(), Err(MLeaderError::BadSize));
        let mut bad = good;
        if let MLeaderContent::MText(t) = &mut bad.content {
            t.text.nominal_height = 0.0;
        }
        assert!(matches!(bad.validate(), Err(MLeaderError::Text(_))));
    }

    #[test]
    fn the_attachment_row_sets_where_the_text_sits() {
        let mut style = MLeaderStyle::standard();
        style.content.left_attachment = LeaderAttachment::MiddleOfText;
        let m = MLeader::build(&style, vec![p(0.0, 0.0), p(10.0, 10.0)], "x").unwrap();
        assert_eq!(m.text().unwrap().text.insertion.y, 10.0);
    }
}
