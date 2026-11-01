//! One line of drawing text (`TEXT` in DXF). The text keeps the placement
//! fields of an attribute because DXF places both in the same way; its
//! [`MText`] projection is the one drawing, picking and plotting use.

use crate::block::AttributeText;
use crate::text_style::TextStyle;
use crate::{MText, Point, TextStyleId};

/// Most text accepted by one single-line object.
pub const MAX_TEXT_BYTES: usize = 16 * 1024;

/// A two-point TEXT placement. Align changes height and width together; Fit
/// keeps the requested height and changes only the width factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlignmentMode {
    Align,
    Fit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAlignment {
    pub mode: TextAlignmentMode,
    pub second: Point,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SingleLineText {
    pub contents: String,
    pub placement: AttributeText,
    pub alignment: Option<TextAlignment>,
}

impl SingleLineText {
    pub fn new(
        position: Point,
        height: f64,
        contents: impl Into<String>,
        style: TextStyleId,
    ) -> Self {
        Self {
            contents: contents.into(),
            placement: AttributeText::new(position, height, style),
            alignment: None,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.placement.validate()?;
        if self.contents.len() > MAX_TEXT_BYTES {
            return Err("the text is too long");
        }
        if self.contents.contains(['\0', '\r', '\n']) {
            return Err("single-line text cannot contain NUL or a line break");
        }
        if let Some(alignment) = self.alignment {
            if self.placement.horizontal != crate::block::TextHorizontal::Left
                || self.placement.vertical != crate::block::TextVertical::Baseline
            {
                return Err("aligned and fit text must use its baseline endpoints");
            }
            if !alignment.second.x.is_finite() || !alignment.second.y.is_finite() {
                return Err("the second text alignment point is not finite");
            }
            if span(self.position(), alignment.second) <= 1e-9 {
                return Err("the text alignment points must be different");
            }
        }
        Ok(())
    }

    /// The equivalent multiline text used by the shared text engine.
    pub fn projection(&self) -> MText {
        self.placement.projection(&self.contents)
    }

    pub fn style(&self) -> TextStyleId {
        self.placement.style
    }

    pub fn position(&self) -> Point {
        self.placement.position
    }

    /// Resolve a two-point placement using the actual font metrics. The
    /// resolved height and width factor are stored as ordinary TEXT fields so
    /// drawing remains context-free and interoperable with DXF readers.
    pub fn resolve_alignment(
        &mut self,
        mode: TextAlignmentMode,
        second: Point,
        style: &TextStyle,
    ) -> Result<(), &'static str> {
        let span = span(self.position(), second);
        if !span.is_finite() || span <= 1e-9 {
            return Err("the text alignment points must be different");
        }
        self.placement.rotation =
            (second.y - self.position().y).atan2(second.x - self.position().x);
        self.placement.horizontal = crate::block::TextHorizontal::Left;
        self.placement.vertical = crate::block::TextVertical::Baseline;
        self.placement.width_factor = 1.0;
        let mut unit = self.clone();
        unit.alignment = None;
        unit.placement.height = 1.0;
        let natural = unit.projection().layout(style).actual_width;
        if !natural.is_finite() || natural <= 1e-12 {
            return Err("the text has no measurable width");
        }
        match mode {
            TextAlignmentMode::Align => self.placement.height = span / natural,
            TextAlignmentMode::Fit => {
                self.placement.width_factor = span / (natural * self.placement.height)
            }
        }
        self.alignment = Some(TextAlignment { mode, second });
        self.validate()
    }

    /// Move either alignment point, preserving the mode's scaling rule.
    pub fn with_alignment_points(&self, first: Point, second: Point) -> Option<Self> {
        let old = self.alignment?;
        let old_span = span(self.position(), old.second);
        let new_span = span(first, second);
        if old_span <= 1e-9 || !new_span.is_finite() || new_span <= 1e-9 {
            return None;
        }
        let mut moved = self.clone();
        moved.placement.position = first;
        moved.placement.rotation = (second.y - first.y).atan2(second.x - first.x);
        match old.mode {
            TextAlignmentMode::Align => moved.placement.height *= new_span / old_span,
            TextAlignmentMode::Fit => moved.placement.width_factor *= new_span / old_span,
        }
        moved.alignment = Some(TextAlignment {
            mode: old.mode,
            second,
        });
        moved.validate().ok().map(|_| moved)
    }
}

fn span(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{TextHorizontal, TextVertical};

    #[test]
    fn projection_keeps_the_text_place_and_justification() {
        let mut text = SingleLineText::new(
            Point::new(3.0, 4.0),
            2.5,
            r"A {brace} \ slash",
            TextStyleId::STANDARD,
        );
        text.placement.rotation = std::f64::consts::FRAC_PI_2;
        text.placement.horizontal = TextHorizontal::Right;
        text.placement.vertical = TextVertical::Top;
        let projected = text.projection();
        assert_eq!(projected.insertion, Point::new(3.0, 4.0));
        assert_eq!(projected.contents, r"A \{brace\} \\ slash");
        assert_eq!(
            projected.attachment,
            crate::mtext::MTextAttachment::TopRight
        );
        assert!(projected.direction.x.abs() < 1e-9);
        assert!((projected.direction.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_line_break_is_not_valid() {
        let text = SingleLineText::new(Point::default(), 2.5, "one\ntwo", TextStyleId::STANDARD);
        assert!(text.validate().is_err());
    }

    #[test]
    fn align_and_fit_resolve_to_the_requested_span() {
        let style = TextStyle::standard();
        for mode in [TextAlignmentMode::Align, TextAlignmentMode::Fit] {
            let mut text =
                SingleLineText::new(Point::default(), 2.0, "Wide words", TextStyleId::STANDARD);
            text.resolve_alignment(mode, Point::new(12.0, 0.0), &style)
                .unwrap();
            let width = text.projection().layout(&style).actual_width;
            assert!((width - 12.0).abs() < 1e-8, "{mode:?}: {width}");
            if mode == TextAlignmentMode::Fit {
                assert_eq!(text.placement.height, 2.0);
            }
        }
    }

    #[test]
    fn moving_an_alignment_grip_preserves_each_modes_rule() {
        let style = TextStyle::standard();
        for mode in [TextAlignmentMode::Align, TextAlignmentMode::Fit] {
            let mut text =
                SingleLineText::new(Point::default(), 2.0, "Words", TextStyleId::STANDARD);
            text.resolve_alignment(mode, Point::new(10.0, 0.0), &style)
                .unwrap();
            let old_height = text.placement.height;
            let old_width_factor = text.placement.width_factor;
            let moved = text
                .with_alignment_points(Point::default(), Point::new(0.0, 20.0))
                .unwrap();
            assert!((moved.placement.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
            match mode {
                TextAlignmentMode::Align => {
                    assert!((moved.placement.height - old_height * 2.0).abs() < 1e-9)
                }
                TextAlignmentMode::Fit => {
                    assert_eq!(moved.placement.height, old_height);
                    assert!((moved.placement.width_factor - old_width_factor * 2.0).abs() < 1e-9);
                }
            }
        }
    }
}
