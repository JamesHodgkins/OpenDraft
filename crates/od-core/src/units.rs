//! A drawing's units: how lengths and angles are shown and typed, as set by
//! AutoCAD's UNITS command (`LUNITS`, `LUPREC`, `AUNITS`, `AUPREC`, `ANGDIR`,
//! `ANGBASE`, `INSUNITS`).
//!
//! The numbers inside the drawing never change with these settings: lengths
//! are drawing units and angles are radians counter-clockwise from east. Units
//! only decide how those numbers are written (`format_*`) and read (`parse_*`).
//! Architectural and engineering lengths take one drawing unit to be one inch.

mod format;
mod parse;

use std::f64::consts::TAU;

pub use format::{format_angle, format_length, format_point};
pub use parse::{parse_angle, parse_length};

/// Most digits (or fraction steps) of precision AutoCAD allows.
pub const MAX_PRECISION: u8 = 8;

/// How lengths are written (`LUNITS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LengthFormat {
    Scientific,
    #[default]
    Decimal,
    Engineering,
    Architectural,
    Fractional,
}

impl LengthFormat {
    pub const ALL: [LengthFormat; 5] = [
        LengthFormat::Scientific,
        LengthFormat::Decimal,
        LengthFormat::Engineering,
        LengthFormat::Architectural,
        LengthFormat::Fractional,
    ];

    /// The `LUNITS` value.
    pub fn code(self) -> i64 {
        self as i64 + 1
    }

    pub fn from_code(code: i64) -> Option<Self> {
        usize::try_from(code - 1)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Scientific => "Scientific",
            Self::Decimal => "Decimal",
            Self::Engineering => "Engineering",
            Self::Architectural => "Architectural",
            Self::Fractional => "Fractional",
        }
    }

    /// Whether precision counts fraction steps (1/2, 1/4...) not decimals.
    pub fn is_fraction(self) -> bool {
        matches!(self, Self::Architectural | Self::Fractional)
    }
}

/// How angles are written (`AUNITS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AngleFormat {
    #[default]
    DecimalDegrees,
    DegreesMinutesSeconds,
    Gradians,
    Radians,
    Surveyor,
}

impl AngleFormat {
    pub const ALL: [AngleFormat; 5] = [
        AngleFormat::DecimalDegrees,
        AngleFormat::DegreesMinutesSeconds,
        AngleFormat::Gradians,
        AngleFormat::Radians,
        AngleFormat::Surveyor,
    ];

    /// The `AUNITS` value.
    pub fn code(self) -> i64 {
        self as i64
    }

    pub fn from_code(code: i64) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DecimalDegrees => "Decimal Degrees",
            Self::DegreesMinutesSeconds => "Deg/Min/Sec",
            Self::Gradians => "Grads",
            Self::Radians => "Radians",
            Self::Surveyor => "Surveyor's Units",
        }
    }
}

/// What one drawing unit is when content is inserted (`INSUNITS`), with
/// AutoCAD's codes. Informational: no scaling is done with it yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InsertUnits(u8);

const INSERT_LABELS: [&str; 21] = [
    "Unitless",
    "Inches",
    "Feet",
    "Miles",
    "Millimeters",
    "Centimeters",
    "Meters",
    "Kilometers",
    "Microinches",
    "Mils",
    "Yards",
    "Angstroms",
    "Nanometers",
    "Microns",
    "Decimeters",
    "Decameters",
    "Hectometers",
    "Gigameters",
    "Astronomical Units",
    "Light Years",
    "Parsecs",
];

/// Short forms of the labels above, as written after a value.
const INSERT_ABBREVIATIONS: [&str; 21] = [
    "", "in", "ft", "mi", "mm", "cm", "m", "km", "\u{b5}in", "mil", "yd", "\u{c5}", "nm",
    "\u{b5}m", "dm", "dam", "hm", "Gm", "AU", "ly", "pc",
];

impl InsertUnits {
    pub const UNITLESS: InsertUnits = InsertUnits(0);

    pub fn from_code(code: i64) -> Option<Self> {
        u8::try_from(code)
            .ok()
            .filter(|&c| usize::from(c) < INSERT_LABELS.len())
            .map(Self)
    }

    /// Every choice, in code order.
    pub fn all() -> impl Iterator<Item = InsertUnits> {
        (0..INSERT_LABELS.len() as u8).map(Self)
    }

    pub fn code(self) -> i64 {
        i64::from(self.0)
    }

    pub fn label(self) -> &'static str {
        INSERT_LABELS[usize::from(self.0)]
    }

    /// `mm`, `ft`...; empty for unitless.
    pub fn abbreviation(self) -> &'static str {
        INSERT_ABBREVIATIONS[usize::from(self.0)]
    }
}

/// The unit settings saved in a drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Units {
    pub length: LengthFormat,
    /// Decimals, or for fractions the denominator 2^n (`LUPREC`, 0 to 8).
    pub length_precision: u8,
    pub angle: AngleFormat,
    /// Decimals of an angle (`AUPREC`, 0 to 8).
    pub angle_precision: u8,
    /// Angles count clockwise (`ANGDIR`).
    pub clockwise: bool,
    /// Where angle 0 points, in radians counter-clockwise from east
    /// (`ANGBASE`).
    pub base_angle: f64,
    pub insert: InsertUnits,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            length: LengthFormat::Decimal,
            length_precision: 4,
            angle: AngleFormat::DecimalDegrees,
            angle_precision: 0,
            clockwise: false,
            base_angle: 0.0,
            insert: InsertUnits::UNITLESS,
        }
    }
}

impl Units {
    /// The same settings with every value in range: precisions at most
    /// [`MAX_PRECISION`], and a finite base angle in `[0, 2π)`.
    pub fn sanitized(mut self) -> Self {
        self.length_precision = self.length_precision.min(MAX_PRECISION);
        self.angle_precision = self.angle_precision.min(MAX_PRECISION);
        self.base_angle = if self.base_angle.is_finite() {
            self.base_angle.rem_euclid(TAU)
        } else {
            0.0
        };
        self
    }

    /// Surveyor's bearings are absolute, so they ignore the base angle and
    /// direction; everything else counts from the base angle.
    fn frame(&self) -> (f64, f64) {
        if self.angle == AngleFormat::Surveyor {
            return (0.0, 1.0);
        }
        (self.base_angle, if self.clockwise { -1.0 } else { 1.0 })
    }

    /// An angle as it reads on screen (radians from the base angle, in the
    /// angle direction), from one counter-clockwise from east.
    pub fn to_display_angle(&self, angle: f64) -> f64 {
        let (base, sign) = self.frame();
        (angle - base) * sign
    }

    /// The angle counter-clockwise from east for one typed in this drawing's
    /// base angle and direction. The reverse of [`Units::to_display_angle`].
    pub fn from_display_angle(&self, shown: f64) -> f64 {
        let (base, sign) = self.frame();
        base + shown * sign
    }

    /// Two lines showing the format at work, as the UNITS dialog does.
    pub fn sample_output(&self) -> [String; 2] {
        let angle = self.from_display_angle(45.0_f64.to_radians());
        [
            format!(
                "{},{}",
                format_length(self, 1.5),
                format_length(self, 2.0039)
            ),
            format!("{}<{}", format_length(self, 3.0), format_angle(self, angle)),
        ]
    }

    /// How a precision choice reads, for the dialog's drop-down.
    pub fn length_precision_label(format: LengthFormat, precision: u8) -> String {
        let sample = Units {
            length: format,
            length_precision: precision,
            ..Units::default()
        };
        let value = if format.is_fraction() && precision > 0 {
            1.0 / f64::from(1_u32 << precision)
        } else {
            0.0
        };
        format_length(&sample, value)
    }

    /// How an angle precision choice reads, for the dialog's drop-down.
    pub fn angle_precision_label(format: AngleFormat, precision: u8) -> String {
        let sample = Units {
            angle: format,
            angle_precision: precision,
            ..Units::default()
        };
        format_angle(&sample, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_autocad() {
        assert_eq!(LengthFormat::Decimal.code(), 2);
        assert_eq!(
            LengthFormat::from_code(4),
            Some(LengthFormat::Architectural)
        );
        assert_eq!(LengthFormat::from_code(0), None);
        assert_eq!(LengthFormat::from_code(6), None);
        assert_eq!(AngleFormat::Surveyor.code(), 4);
        assert_eq!(
            AngleFormat::from_code(1),
            Some(AngleFormat::DegreesMinutesSeconds)
        );
        assert_eq!(AngleFormat::from_code(-1), None);
        assert_eq!(
            InsertUnits::from_code(4).map(InsertUnits::label),
            Some("Millimeters")
        );
        assert_eq!(InsertUnits::from_code(21), None);
        assert_eq!(InsertUnits::all().count(), 21);
        assert!(
            InsertUnits::all().all(|u| u == InsertUnits::UNITLESS || !u.abbreviation().is_empty())
        );
    }

    #[test]
    fn sanitizing_clamps_precision_and_base_angle() {
        let wild = Units {
            length_precision: 99,
            angle_precision: 12,
            base_angle: f64::NAN,
            ..Units::default()
        };
        let clean = wild.sanitized();
        assert_eq!((clean.length_precision, clean.angle_precision), (8, 8));
        assert_eq!(clean.base_angle, 0.0);
        let wrapped = Units {
            base_angle: -std::f64::consts::FRAC_PI_2,
            ..Units::default()
        }
        .sanitized();
        assert!((wrapped.base_angle - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn display_angles_follow_the_base_angle_and_direction() {
        let units = Units {
            base_angle: std::f64::consts::FRAC_PI_2,
            clockwise: true,
            ..Units::default()
        };
        // North is the base angle, so east is a quarter turn clockwise from it.
        let shown = units.to_display_angle(0.0);
        assert!((shown - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(units.from_display_angle(shown).abs() < 1e-12);
        let plain = Units::default();
        assert_eq!(plain.to_display_angle(1.25), 1.25);
    }

    #[test]
    fn the_sample_output_matches_autocads() {
        let [point, polar] = Units::default().sample_output();
        assert_eq!(point, "1.5000,2.0039");
        assert_eq!(polar, "3.0000<45");
    }

    #[test]
    fn precision_labels_show_the_format() {
        let label = Units::length_precision_label;
        assert_eq!(label(LengthFormat::Decimal, 2), "0.00");
        assert_eq!(label(LengthFormat::Architectural, 1), "0'-0 1/2\"");
        assert_eq!(label(LengthFormat::Fractional, 0), "0");
        let angle = Units::angle_precision_label;
        assert_eq!(angle(AngleFormat::DegreesMinutesSeconds, 2), "0d00'00\"");
        assert_eq!(angle(AngleFormat::DecimalDegrees, 3), "0.000");
    }
}
