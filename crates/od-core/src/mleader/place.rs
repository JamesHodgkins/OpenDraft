//! Where a multileader's text sits against the end of its landing.

use super::style::LeaderAttachment;
use crate::mtext::MTextAttachment;
use crate::Point;

/// The attachment of the text and its insertion point, for text whose edge
/// meets the landing ending at `end`. `leader_on_left` puts the text to the
/// right of `end` (and so on the text's left edge), `gap` apart from it;
/// `height` is the text height, which sets how far the row the landing joins
/// is from the text's top or bottom.
pub fn place_text(
    attachment: LeaderAttachment,
    leader_on_left: bool,
    end: Point,
    gap: f64,
    height: f64,
) -> (MTextAttachment, Point) {
    use LeaderAttachment::*;
    use MTextAttachment::*;
    // The text's own row, and how far its anchor is above the landing.
    let (row, lift) = match attachment {
        TopOfTopLine => (0, 0.0),
        MiddleOfTopLine => (0, height / 2.0),
        MiddleOfText => (1, 0.0),
        MiddleOfBottomLine => (2, -height / 2.0),
        BottomOfBottomLine | BottomLine | UnderlineBottomLine | UnderlineAllText => (2, 0.0),
        UnderlineTopLine => (0, height),
    };
    let side = if leader_on_left { 0 } else { 1 };
    let kind = [
        [TopLeft, TopRight],
        [MiddleLeft, MiddleRight],
        [BottomLeft, BottomRight],
    ][row][side];
    let x = if leader_on_left {
        end.x + gap
    } else {
        end.x - gap
    };
    (kind, Point::new(x, end.y + lift))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_the_landing_joins_sets_the_anchor() {
        let end = Point::new(10.0, 5.0);
        let (kind, at) = place_text(LeaderAttachment::MiddleOfTopLine, true, end, 2.0, 2.5);
        assert_eq!(kind, MTextAttachment::TopLeft);
        assert_eq!((at.x, at.y), (12.0, 6.25));
        let (kind, at) = place_text(LeaderAttachment::MiddleOfBottomLine, false, end, 2.0, 2.5);
        assert_eq!(kind, MTextAttachment::BottomRight);
        assert_eq!((at.x, at.y), (8.0, 3.75));
        let (kind, at) = place_text(LeaderAttachment::MiddleOfText, true, end, 0.0, 2.5);
        assert_eq!(kind, MTextAttachment::MiddleLeft);
        assert_eq!(at, end);
    }
}
