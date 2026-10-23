//! Dimensions: aligned, rotated (horizontal and vertical), radius, diameter
//! and arc length. Modelled on AutoCAD's: the definition points, the style
//! and the text are the dimension, and the lines, arrows and text that show
//! it ([`Dimension::picture`]) are worked out from them every time, so editing
//! a point always redraws the whole thing.
//!
//! A dimension keeps a copy of the style it was made with ([`DimStyle`]).
//! Everything is pure geometry; the commands only ask for points.

mod angular;
mod arc_length;
mod centre;
mod chain;
mod edit;
mod fit;
pub mod font;
mod jogged;
mod jogline;
mod linear;
mod ordinate;
mod pick;
mod radial;
mod sketch;
mod style;
mod text;
mod tolerance;

use crate::{Bounds, Entity, Point};

pub use centre::centre_segments;
pub use jogline::spaced_offsets;
pub use ordinate::suggested_x_datum;
pub use pick::{auto_rotation, lines_angle_points, object_ends};
pub use style::{Arrow, DimStyle};
pub use text::{angle_text, expand_codes, measurement_text, DimText};

/// An arrowhead of `kind` as ordinary entities, its point at `tip` and `back`
/// the unit direction from the tip along the body (what a multileader's
/// arrow is drawn with too).
pub fn arrow_pieces(kind: Arrow, tip: Point, back: (f64, f64), size: f64) -> Vec<Entity> {
    let mut sketch = sketch::Sketch::default();
    sketch.arrow(kind, tip, back, size);
    sketch.pieces
}

thread_local! {
    static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many dimension pictures this thread has built, for tests that check a
/// picture is remembered rather than rebuilt (a count per thread, so tests
/// running side by side do not disturb one another).
pub fn builds_on_this_thread() -> usize {
    BUILDS.with(std::cell::Cell::get)
}

/// What is being measured, and the points that define it.
#[derive(Clone, Debug, PartialEq)]
pub enum DimKind {
    /// The distance between two points. `rotation` is the angle of the
    /// dimension line, or `None` to run it parallel to the two points
    /// (aligned); `oblique` is the angle of the extension lines, or `None`
    /// to stand them square to the dimension line.
    Linear {
        p1: Point,
        p2: Point,
        /// A point on the dimension line.
        line_point: Point,
        rotation: Option<f64>,
        oblique: Option<f64>,
    },
    /// The radius or diameter of the circle through `chord_point` about
    /// `center`. `leader_length` is how far the line runs past the circle
    /// when the text sits outside.
    Radial {
        center: Point,
        chord_point: Point,
        diameter: bool,
        leader_length: f64,
        /// A jogged radius dimension: where the centre is shown to be
        /// (the line heads there) and where the jog is.
        jog: Option<(Point, Point)>,
    },
    /// The length of the arc from `start` to `end` counter-clockwise about
    /// `center`; the dimension arc passes through `arc_point`.
    ArcLength {
        center: Point,
        start: Point,
        end: Point,
        arc_point: Point,
        /// A radial leader from the dimension arc to the measured arc, drawn
        /// for an arc of more than a quarter turn.
        leader: bool,
    },
    /// The angle between two rays out of `vertex`, through `p1` and `p2`,
    /// drawn as an arc through `arc_point`. `lines` says the rays are whole
    /// lines (four angles to choose from) rather than rays.
    Angular {
        vertex: Point,
        p1: Point,
        p2: Point,
        arc_point: Point,
        lines: bool,
    },
    /// The X (`x_datum`) or Y coordinate of `feature`, on a leader to
    /// `leader_end`.
    Ordinate {
        feature: Point,
        leader_end: Point,
        x_datum: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dimension {
    pub kind: DimKind,
    pub style: DimStyle,
    pub text: DimText,
    /// Where the user put the middle of the text; `None` lets the style place
    /// it.
    pub text_pos: Option<Point>,
    /// The angle the user turned the text to (radians, from the x axis);
    /// `None` lets the style decide.
    pub text_angle: Option<f64>,
    /// Where a jog line is put in a linear dimension's line.
    pub jog_line: Option<Point>,
}

/// The dimension drawn: its pieces and where the text's middle is.
pub(crate) struct Built {
    pub pieces: Vec<Entity>,
    pub text_centre: Point,
}

impl Dimension {
    fn with(kind: DimKind, style: &DimStyle) -> Dimension {
        Dimension {
            kind,
            style: style.clone().sanitized(),
            text: DimText::Measured,
            text_pos: None,
            text_angle: None,
            jog_line: None,
        }
    }

    /// Parallel to the line through `p1` and `p2`.
    pub fn aligned(p1: Point, p2: Point, line_point: Point, style: &DimStyle) -> Dimension {
        Self::with(
            DimKind::Linear {
                p1,
                p2,
                line_point,
                rotation: None,
                oblique: None,
            },
            style,
        )
    }

    /// With the dimension line at `rotation` radians (0 horizontal, a
    /// quarter turn vertical).
    pub fn linear(
        p1: Point,
        p2: Point,
        line_point: Point,
        rotation: f64,
        style: &DimStyle,
    ) -> Dimension {
        Self::with(
            DimKind::Linear {
                p1,
                p2,
                line_point,
                rotation: Some(rotation),
                oblique: None,
            },
            style,
        )
    }

    pub fn radius(center: Point, chord_point: Point, style: &DimStyle) -> Dimension {
        Self::radial(center, chord_point, false, style)
    }

    pub fn diameter(center: Point, chord_point: Point, style: &DimStyle) -> Dimension {
        Self::radial(center, chord_point, true, style)
    }

    fn radial(center: Point, chord_point: Point, diameter: bool, style: &DimStyle) -> Dimension {
        let leader_length = style.arrow_size * style.scale;
        Self::with(
            DimKind::Radial {
                center,
                chord_point,
                diameter,
                leader_length,
                jog: None,
            },
            style,
        )
    }

    /// A radius dimension of the circle through `chord_point` about `center`
    /// drawn as if its centre were at `shown_centre`, with the jog at `jog`.
    pub fn jogged(
        center: Point,
        chord_point: Point,
        shown_centre: Point,
        jog: Point,
        style: &DimStyle,
    ) -> Dimension {
        let mut out = Self::radius(center, chord_point, style);
        if let DimKind::Radial { jog: j, .. } = &mut out.kind {
            *j = Some((shown_centre, jog));
        }
        out
    }

    /// The angle at `vertex` between the directions to `p1` and `p2`.
    pub fn angular(
        vertex: Point,
        p1: Point,
        p2: Point,
        arc_point: Point,
        lines: bool,
        style: &DimStyle,
    ) -> Dimension {
        Self::with(
            DimKind::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                lines,
            },
            style,
        )
    }

    /// The X or Y coordinate of `feature`, led to `leader_end`.
    pub fn ordinate(
        feature: Point,
        leader_end: Point,
        x_datum: bool,
        style: &DimStyle,
    ) -> Dimension {
        Self::with(
            DimKind::Ordinate {
                feature,
                leader_end,
                x_datum,
            },
            style,
        )
    }

    pub fn arc_length(
        center: Point,
        start: Point,
        end: Point,
        arc_point: Point,
        style: &DimStyle,
    ) -> Dimension {
        Self::with(
            DimKind::ArcLength {
                center,
                start,
                end,
                arc_point,
                leader: false,
            },
            style,
        )
    }

    /// What is measured, in drawing units, before `DIMLFAC` and rounding.
    /// `None` when the points do not define one.
    pub fn measurement(&self) -> Option<f64> {
        let finite = |v: f64| v.is_finite().then_some(v);
        match &self.kind {
            DimKind::Linear { .. } => finite(linear::measurement(&self.kind)?),
            DimKind::Radial {
                center,
                chord_point,
                diameter,
                ..
            } => {
                let r = sketch::dist(*center, *chord_point);
                (r > 1e-12).then_some(if *diameter { 2.0 * r } else { r })
            }
            DimKind::ArcLength { .. } => finite(arc_length::measurement(&self.kind)?),
            DimKind::Angular { .. } => finite(angular::measurement(&self.kind)?),
            DimKind::Ordinate { .. } => finite(ordinate::measurement(&self.kind)?),
        }
    }

    /// The sign written before the number: `R` for a radius, `Ø` for a
    /// diameter.
    fn prefix(&self) -> &'static str {
        match &self.kind {
            DimKind::Radial { diameter: true, .. } => "Ø",
            DimKind::Radial { .. } => "R",
            _ => "",
        }
    }

    /// The text the dimension shows (empty when hidden or degenerate).
    pub fn label(&self) -> String {
        text::shown(&self.text, &self.measured_label())
    }

    pub(crate) fn build(&self) -> Option<Built> {
        BUILDS.with(|n| n.set(n.get() + 1));
        match &self.kind {
            DimKind::Linear { .. } => linear::build(self),
            DimKind::Radial { .. } => radial::build(self),
            DimKind::ArcLength { .. } => arc_length::build(self),
            DimKind::Angular { .. } => angular::build(self),
            DimKind::Ordinate { .. } => ordinate::build(self),
        }
    }

    /// The lines, arcs, arrowheads and text strokes that draw it. Arrowheads
    /// are tapering wide polylines (filled triangles). Empty when the points
    /// do not define a dimension.
    pub fn picture(&self) -> Vec<Entity> {
        self.build().map(|b| b.pieces).unwrap_or_default()
    }

    /// The middle of the text as drawn.
    pub fn text_centre(&self) -> Option<Point> {
        self.build().map(|b| b.text_centre)
    }

    pub fn bounds(&self) -> Bounds {
        self.bounds_of(&self.picture())
    }

    /// The box around `picture` (this dimension's pieces, from
    /// [`Dimension::picture`] or an object's remembered copy); for a
    /// dimension that draws nothing, its first defining point.
    pub fn bounds_of(&self, picture: &[Entity]) -> Bounds {
        let mut all = picture.iter().map(Entity::bounds);
        match all.next() {
            Some(first) => all.fold(first, Bounds::union),
            None => {
                let at = self.points().first().copied().unwrap_or_default();
                Bounds::from_points(at, at)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn radial_measurements() {
        let style = DimStyle::default();
        let r = Dimension::radius(p(0.0, 0.0), p(3.0, 4.0), &style);
        assert_eq!(r.measurement(), Some(5.0));
        let d = Dimension::diameter(p(0.0, 0.0), p(3.0, 4.0), &style);
        assert_eq!(d.measurement(), Some(10.0));
    }

    #[test]
    fn labels_carry_the_radius_and_diameter_signs() {
        let style = DimStyle::default();
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &style);
        assert_eq!(r.label(), "R5.00");
        let d = Dimension::diameter(p(0.0, 0.0), p(5.0, 0.0), &style);
        assert_eq!(d.label(), "Ø10.00");
    }

    #[test]
    fn the_words_carry_the_styles_text_style() {
        let style = DimStyle {
            text_style: crate::TextStyleId(4),
            ..DimStyle::default()
        };
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &style);
        let texts: Vec<_> = d
            .picture()
            .into_iter()
            .filter_map(|e| match e {
                Entity::MText(t) => Some(t.style),
                _ => None,
            })
            .collect();
        assert_eq!(texts, [crate::TextStyleId(4)]);
    }

    #[test]
    fn typed_and_hidden_text() {
        let style = DimStyle::default();
        let mut d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &style);
        assert_eq!(d.label(), "10.00");
        d.text = DimText::Custom("<> TYP".to_owned());
        assert_eq!(d.label(), "10.00 TYP");
        d.text = DimText::Hidden;
        assert_eq!(d.label(), "");
    }

    #[test]
    fn a_degenerate_dimension_draws_nothing_and_does_not_panic() {
        let style = DimStyle::default();
        let d = Dimension::radius(p(1.0, 1.0), p(1.0, 1.0), &style);
        assert!(d.picture().is_empty());
        assert_eq!(d.measurement(), None);
        let b = d.bounds();
        assert_eq!(b.min, b.max);
    }

    #[test]
    fn bounds_cover_the_picture() {
        let style = DimStyle::default();
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &style);
        let b = d.bounds();
        assert!(b.min.x <= 0.0 && b.max.x >= 10.0);
        assert!(b.max.y >= 5.0);
    }
}
