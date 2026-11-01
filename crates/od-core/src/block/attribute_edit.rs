//! Moving, turning, mirroring and scaling attribute text. As with multiline
//! text, a reflection moves the text to its reflected place and never
//! reverses it (AutoCAD's MIRRTEXT of 0).

use crate::block::attribute::{AttributeDefinition, AttributeText};
use crate::{arc, Point};

impl AttributeText {
    pub fn translated(&self, dx: f64, dy: f64) -> AttributeText {
        AttributeText {
            position: self.position.translated(dx, dy),
            ..self.clone()
        }
    }

    pub fn rotated(&self, center: Point, angle: f64) -> AttributeText {
        AttributeText {
            position: self.position.rotated(center, angle),
            rotation: arc::normalize(self.rotation + angle),
            ..self.clone()
        }
    }

    pub fn mirrored(&self, a: Point, b: Point) -> AttributeText {
        AttributeText {
            position: self.position.mirrored(a, b),
            ..self.clone()
        }
    }

    pub fn scaled(&self, base: Point, factor: f64) -> AttributeText {
        AttributeText {
            position: self.position.scaled(base, factor),
            height: self.height * factor.abs(),
            ..self.clone()
        }
    }
}

impl AttributeDefinition {
    fn with_text(&self, text: AttributeText) -> AttributeDefinition {
        AttributeDefinition {
            text,
            ..self.clone()
        }
    }

    pub fn translated(&self, dx: f64, dy: f64) -> AttributeDefinition {
        self.with_text(self.text.translated(dx, dy))
    }

    pub fn rotated(&self, center: Point, angle: f64) -> AttributeDefinition {
        self.with_text(self.text.rotated(center, angle))
    }

    pub fn mirrored(&self, a: Point, b: Point) -> AttributeDefinition {
        self.with_text(self.text.mirrored(a, b))
    }

    pub fn scaled(&self, base: Point, factor: f64) -> AttributeDefinition {
        self.with_text(self.text.scaled(base, factor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextStyleId;
    use std::f64::consts::FRAC_PI_2;

    fn text() -> AttributeText {
        AttributeText::new(Point::new(2.0, 0.0), 1.0, TextStyleId::STANDARD)
    }

    #[test]
    fn turning_moves_the_anchor_and_adds_to_the_rotation() {
        let t = text().rotated(Point::default(), FRAC_PI_2);
        assert!(t.position.x.abs() < 1e-12 && (t.position.y - 2.0).abs() < 1e-12);
        assert!((t.rotation - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn mirroring_keeps_the_text_readable() {
        let t = text().mirrored(Point::new(0.0, -1.0), Point::new(0.0, 1.0));
        assert!((t.position.x + 2.0).abs() < 1e-12);
        assert_eq!(t.rotation, 0.0);
    }

    #[test]
    fn scaling_scales_the_height_and_the_distance() {
        let t = text().scaled(Point::default(), 3.0);
        assert_eq!((t.position.x, t.height), (6.0, 3.0));
    }
}
