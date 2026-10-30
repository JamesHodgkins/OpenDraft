//! A weld neck flange, drawn as an orthographic pair: a section through the
//! axis (the axis horizontal, the face at the left and the weld end at the
//! right) and the face view (the circles seen along the axis), placed to the
//! left of the section as third-angle projection puts a view of the left end.
//! Both come from the same numbers, so they agree.
//!
//! The origin is where the raised face meets the axis. All lengths are in the
//! drawing's units. The recipe draws what it is given: it does not know the
//! standard, so a class and size must be turned into these numbers by the
//! caller (which is why every number has a name a standard's table uses).
//!
//! Conventions: `thickness` is from the flange face (the surface the raised
//! face stands on) to the back of the flange; `overall_length` is from the
//! raised face to the weld end of the hub; the hub tapers in a straight line
//! from `hub_diameter_base` at the back of the flange to `hub_diameter_end` at
//! the weld end. The weld bevel is not drawn. With an even number of bolt
//! holes they straddle the centrelines, as flange standards require; an odd
//! number cannot, so one hole is at the top.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::{positive, PartDrawing, PartError, Role, Shape};
use crate::Point;

#[derive(Clone, Debug, PartialEq)]
pub struct WeldNeckFlange {
    pub outside_diameter: f64,
    pub thickness: f64,
    /// Diameter of the raised face; ignored when its height is 0.
    pub raised_face_diameter: f64,
    /// 0 for a flat face.
    pub raised_face_height: f64,
    pub hub_diameter_base: f64,
    pub hub_diameter_end: f64,
    pub overall_length: f64,
    pub bore: f64,
    pub bolt_circle_diameter: f64,
    pub bolt_count: u32,
    pub bolt_hole_diameter: f64,
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

impl WeldNeckFlange {
    /// Whether the numbers describe a flange that can exist: the diameters
    /// nest, the holes clear the hub and the edge, and the lengths add up.
    pub fn validate(&self) -> Result<(), PartError> {
        positive(&[
            ("outside_diameter", self.outside_diameter),
            ("thickness", self.thickness),
            ("hub_diameter_base", self.hub_diameter_base),
            ("hub_diameter_end", self.hub_diameter_end),
            ("overall_length", self.overall_length),
            ("bore", self.bore),
            ("bolt_circle_diameter", self.bolt_circle_diameter),
            ("bolt_hole_diameter", self.bolt_hole_diameter),
        ])?;
        let raised = self.raised_face_height;
        if !raised.is_finite() || raised < 0.0 || raised > 1.0e7 {
            return Err(PartError(format!(
                "raised_face_height must be 0 or more (it was {raised})."
            )));
        }
        if raised > 0.0 {
            positive(&[("raised_face_diameter", self.raised_face_diameter)])?;
        }
        if !(1..=64).contains(&self.bolt_count) {
            return Err(PartError(format!(
                "bolt_count must be from 1 to 64 (it was {}).",
                self.bolt_count
            )));
        }
        let (o, b, a, x) = (
            self.outside_diameter,
            self.bore,
            self.hub_diameter_end,
            self.hub_diameter_base,
        );
        if !(b < a && a <= x && x < o) {
            return Err(PartError(format!(
                "The diameters must nest: bore ({b}) < hub_diameter_end ({a}) <= \
                 hub_diameter_base ({x}) < outside_diameter ({o})."
            )));
        }
        if raised > 0.0 && !(b < self.raised_face_diameter && self.raised_face_diameter < o) {
            return Err(PartError(format!(
                "raised_face_diameter ({}) must be between the bore ({b}) and the outside \
                 diameter ({o}).",
                self.raised_face_diameter
            )));
        }
        if self.overall_length <= raised + self.thickness {
            return Err(PartError(format!(
                "overall_length ({}) must be more than raised_face_height + thickness ({}) so \
                 the hub has a length.",
                self.overall_length,
                raised + self.thickness
            )));
        }
        let (w, h) = (self.bolt_circle_diameter, self.bolt_hole_diameter);
        if (w - h) / 2.0 <= x / 2.0 {
            return Err(PartError(format!(
                "The bolt holes do not clear the hub: bolt_circle_diameter ({w}) minus \
                 bolt_hole_diameter ({h}), halved, must exceed hub_diameter_base ({x}) halved."
            )));
        }
        if (w + h) / 2.0 >= o / 2.0 {
            return Err(PartError(format!(
                "The bolt holes run off the edge: bolt_circle_diameter ({w}) plus \
                 bolt_hole_diameter ({h}) must be less than outside_diameter ({o})."
            )));
        }
        if raised > 0.0 && self.raised_face_diameter > w - h {
            return Err(PartError(format!(
                "The raised face ({}) must sit inside the bolt holes (bolt circle {w} minus \
                 hole {h}).",
                self.raised_face_diameter
            )));
        }
        Ok(())
    }

    /// The section and the face view, as exact shapes about the origin.
    pub fn shapes(&self) -> Result<Vec<Shape>, PartError> {
        self.drawing().map(|d| d.shapes)
    }

    /// The shapes with what each is for: the section outlines (which a
    /// section hatch fills, with lines spaced an eighth of the flange
    /// thickness apart), the face view's circles, and the centre lines with
    /// the bolt circle.
    pub fn drawing(&self) -> Result<PartDrawing, PartError> {
        self.validate()?;
        let mut pieces: Vec<(Role, Shape)> = self
            .section()
            .into_iter()
            .map(|s| (Role::Outline, s))
            .collect();
        pieces.extend(self.face_view());
        pieces.extend(self.centrelines().into_iter().map(|s| (Role::Centre, s)));
        let (roles, shapes) = pieces.into_iter().unzip();
        Ok(PartDrawing {
            shapes,
            roles,
            hatched: vec![0, 1],
            hatch_spacing: self.thickness / 8.0,
        })
    }

    /// Where the face view's centre is: left of the section by a quarter of
    /// the outside diameter, so the views do not touch.
    fn face_centre(&self) -> f64 {
        -(self.outside_diameter * 0.25 + self.outside_diameter / 2.0)
    }

    /// The two halves of the section either side of the bore. The upper half
    /// goes from the bore at the face round the raised face, the flange, the
    /// hub and back along the bore; the lower half is its mirror.
    fn section(&self) -> Vec<Shape> {
        let (o, b) = (self.outside_diameter / 2.0, self.bore / 2.0);
        let (a, x) = (self.hub_diameter_end / 2.0, self.hub_diameter_base / 2.0);
        let f = self.raised_face_height;
        let back = f + self.thickness;
        let mut upper = vec![p(0.0, b)];
        if f > 0.0 {
            let r = self.raised_face_diameter / 2.0;
            upper.extend([p(0.0, r), p(f, r)]);
        }
        upper.extend([
            p(f, o),
            p(back, o),
            p(back, x),
            p(self.overall_length, a),
            p(self.overall_length, b),
        ]);
        let lower = upper.iter().map(|q| p(q.x, -q.y)).collect();
        vec![Shape::Outline(upper), Shape::Outline(lower)]
    }

    /// The circles seen from the face: the edge, the raised face, the bore
    /// and the bolt holes (and the circle they sit on).
    fn face_view(&self) -> Vec<(Role, Shape)> {
        let c = p(self.face_centre(), 0.0);
        let circle = |radius: f64| Shape::Circle { center: c, radius };
        let mut shapes = vec![(Role::Outline, circle(self.outside_diameter / 2.0))];
        if self.raised_face_height > 0.0 {
            shapes.push((Role::Outline, circle(self.raised_face_diameter / 2.0)));
        }
        shapes.push((Role::Outline, circle(self.bore / 2.0)));
        shapes.push((Role::Centre, circle(self.bolt_circle_diameter / 2.0)));
        let n = f64::from(self.bolt_count);
        // An even count straddles both centrelines (half a step either side
        // of the vertical). An odd count cannot, so one hole is at the top
        // and the pattern is symmetric about the vertical only.
        let first = if self.bolt_count.is_multiple_of(2) {
            FRAC_PI_2 + TAU / n / 2.0
        } else {
            FRAC_PI_2
        };
        for k in 0..self.bolt_count {
            let angle = first + TAU * f64::from(k) / n;
            let r = self.bolt_circle_diameter / 2.0;
            shapes.push((
                Role::Outline,
                Shape::Circle {
                    center: p(c.x + r * angle.cos(), r * angle.sin()),
                    radius: self.bolt_hole_diameter / 2.0,
                },
            ));
        }
        shapes
    }

    /// The axis through both views, and the vertical centreline of the face
    /// view, each running a tenth of the outside diameter past the part.
    fn centrelines(&self) -> Vec<Shape> {
        let reach = self.outside_diameter / 10.0;
        let o = self.outside_diameter / 2.0;
        let cx = self.face_centre();
        vec![
            Shape::Line(p(cx - o - reach, 0.0), p(self.overall_length + reach, 0.0)),
            Shape::Line(p(cx, -o - reach), p(cx, o + reach)),
        ]
    }
}
