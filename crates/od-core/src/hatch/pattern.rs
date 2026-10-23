//! Making and changing pattern fills. A pattern's lines are stored final
//! (see the module notes in [`super`]), so these functions are where scale,
//! angle and origin are applied.

use super::catalog::PatternDef;
use super::{HatchPattern, PatternLine, PatternType};
use crate::Point;

/// What a pattern cannot be made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternError {
    /// The scale or spacing is zero, negative or not a number.
    BadScale,
    /// The angle or origin is not a number.
    NotFinite,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadScale => write!(f, "the scale must be a positive number"),
            Self::NotFinite => write!(f, "the angle and origin must be numbers"),
        }
    }
}

fn turned(v: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

fn check(scale: f64, angle: f64, origin: Point) -> Result<(), PatternError> {
    if !(scale.is_finite() && scale > 0.0) {
        return Err(PatternError::BadScale);
    }
    if !(angle.is_finite() && origin.x.is_finite() && origin.y.is_finite()) {
        return Err(PatternError::NotFinite);
    }
    Ok(())
}

impl HatchPattern {
    /// A named pattern from its definition: each line scaled by `scale`,
    /// the whole turned by `angle` (radians) about `origin`, which is where
    /// the definition's own origin lands.
    pub fn from_definition(
        def: &PatternDef,
        pattern_type: PatternType,
        scale: f64,
        angle: f64,
        origin: Point,
    ) -> Result<Self, PatternError> {
        check(scale, angle, origin)?;
        let lines = def
            .lines
            .iter()
            .map(|l| {
                let own = l.angle.to_radians();
                // The step is (delta-x along the line, delta-y across it).
                let step = turned([l.delta.0, l.delta.1], own);
                let base = turned([l.origin.0 * scale, l.origin.1 * scale], angle);
                PatternLine {
                    angle: own + angle,
                    base: Point::new(origin.x + base[0], origin.y + base[1]),
                    offset: turned([step[0] * scale, step[1] * scale], angle),
                    dashes: l.dashes.iter().map(|d| d * scale).collect(),
                }
            })
            .collect();
        Ok(Self {
            pattern_type,
            name: def.name.clone(),
            angle,
            scale_or_spacing: scale,
            double: false,
            lines,
            origin,
        })
    }

    /// A user-defined pattern: parallel lines `spacing` apart at `angle`
    /// (radians), and with `double` a second set across them.
    pub fn user_defined(
        spacing: f64,
        angle: f64,
        double: bool,
        origin: Point,
    ) -> Result<Self, PatternError> {
        check(spacing, angle, origin)?;
        let family = |a: f64| PatternLine {
            angle: a,
            base: origin,
            offset: turned([0.0, spacing], a),
            dashes: Vec::new(),
        };
        let mut lines = vec![family(angle)];
        if double {
            lines.push(family(angle + std::f64::consts::FRAC_PI_2));
        }
        Ok(Self {
            pattern_type: PatternType::UserDefined,
            name: "_USER".to_owned(),
            angle,
            scale_or_spacing: spacing,
            double,
            lines,
            origin,
        })
    }

    /// This pattern drawn `factor` times as large about its origin.
    pub fn scaled_about_origin(&self, factor: f64) -> Self {
        let o = self.origin;
        Self {
            scale_or_spacing: self.scale_or_spacing * factor,
            lines: self
                .lines
                .iter()
                .map(|l| PatternLine {
                    angle: l.angle,
                    base: Point::new(
                        o.x + (l.base.x - o.x) * factor,
                        o.y + (l.base.y - o.y) * factor,
                    ),
                    offset: [l.offset[0] * factor, l.offset[1] * factor],
                    dashes: l.dashes.iter().map(|d| d * factor).collect(),
                })
                .collect(),
            ..self.clone()
        }
    }

    /// This pattern turned by `delta` radians about its origin.
    pub fn turned_about_origin(&self, delta: f64) -> Self {
        let o = self.origin;
        Self {
            angle: self.angle + delta,
            lines: self
                .lines
                .iter()
                .map(|l| PatternLine {
                    angle: l.angle + delta,
                    base: l.base.rotated(o, delta),
                    offset: turned(l.offset, delta),
                    dashes: l.dashes.clone(),
                })
                .collect(),
            ..self.clone()
        }
    }

    /// The same pattern at another scale (or spacing), if that is a scale.
    pub fn with_scale(&self, scale: f64) -> Result<Self, PatternError> {
        check(scale, 0.0, self.origin)?;
        if self.scale_or_spacing.is_nan() || self.scale_or_spacing <= 0.0 {
            return Err(PatternError::BadScale);
        }
        Ok(self.scaled_about_origin(scale / self.scale_or_spacing))
    }

    /// The same pattern at another angle (radians).
    pub fn with_angle(&self, angle: f64) -> Result<Self, PatternError> {
        check(1.0, angle, self.origin)?;
        Ok(self.turned_about_origin(angle - self.angle))
    }

    /// The same pattern anchored at another origin.
    pub fn with_origin(&self, origin: Point) -> Result<Self, PatternError> {
        check(1.0, 0.0, origin)?;
        let (dx, dy) = (origin.x - self.origin.x, origin.y - self.origin.y);
        Ok(Self {
            origin,
            lines: self
                .lines
                .iter()
                .map(|l| PatternLine {
                    base: l.base.translated(dx, dy),
                    ..l.clone()
                })
                .collect(),
            ..self.clone()
        })
    }
}
