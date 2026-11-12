//! How the drawing shows its point objects.

use super::Document;
use crate::PointStyle;

impl Document {
    pub fn point_style(&self) -> PointStyle {
        self.point_style
    }

    /// Set `PDMODE` and `PDSIZE` and return the old style. A mode `PDMODE`
    /// does not allow, or a size that is not a finite number, is ignored.
    pub fn set_point_style(&mut self, style: PointStyle) -> PointStyle {
        let old = self.point_style;
        if PointStyle::valid_mode(style.mode) && style.size.is_finite() {
            self.point_style = style;
        }
        old
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_that_is_not_allowed_is_ignored() {
        let mut doc = Document::default();
        let old = doc.set_point_style(PointStyle { mode: 3, size: 2.0 });
        assert_eq!(old, PointStyle::default());
        doc.set_point_style(PointStyle { mode: 9, size: 1.0 });
        doc.set_point_style(PointStyle {
            mode: 2,
            size: f64::NAN,
        });
        assert_eq!(doc.point_style().mode, 3);
        assert_eq!(doc.point_style().size, 2.0);
    }
}
