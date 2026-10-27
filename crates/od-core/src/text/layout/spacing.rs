//! A paragraph's own spacing: room before and after it, and its own
//! line-spacing multiple (`\pxb1,a1.5,sm1.1;`).

use crate::text::paragraph::ParagraphStyle;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Spacing {
    pub before: f64,
    pub after: f64,
    /// Replaces the text's line-spacing factor inside the paragraph.
    pub factor: Option<f64>,
}

pub(super) fn spacing_of(style: Option<&ParagraphStyle>) -> Spacing {
    style.map_or_else(Spacing::default, |s| Spacing {
        before: s.space_before.max(0.0),
        after: s.space_after.max(0.0),
        factor: s.line_spacing,
    })
}

/// The room added above a line, beyond the line pitch: the space after the
/// paragraph that ended on the line above, and the space before this line's
/// paragraph if it starts one. None above the first line of the text.
pub(super) fn gap_above(above: Option<(bool, Spacing)>, starts: bool, own: Spacing) -> f64 {
    let Some((above_ended, above)) = above else {
        return 0.0;
    };
    let after = if above_ended { above.after } else { 0.0 };
    let before = if starts { own.before } else { 0.0 };
    after + before
}
