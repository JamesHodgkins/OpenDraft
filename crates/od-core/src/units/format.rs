//! Writing lengths and angles in a drawing's units.

use std::f64::consts::TAU;

use super::{AngleFormat, LengthFormat, Units, MAX_PRECISION};
use crate::Point;

/// A length as the drawing's units write it: `12.5000`, `1.2500E+01`,
/// `1'-0.5000"`, `1'-0 1/2"` or `12 1/2`.
pub fn format_length(units: &Units, value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let precision = units.length_precision.min(MAX_PRECISION);
    let p = usize::from(precision);
    match units.length {
        LengthFormat::Decimal => unsigned_zero(format!("{value:.p$}")),
        LengthFormat::Scientific => scientific(value, p),
        LengthFormat::Engineering => engineering(value, p),
        LengthFormat::Architectural | LengthFormat::Fractional => {
            match Steps::new(value.abs(), precision) {
                Some(steps) => steps.text(value < 0.0, units.length == LengthFormat::Architectural),
                None => format!("{value}"),
            }
        }
    }
}

/// A point as `x,y`.
pub fn format_point(units: &Units, point: Point) -> String {
    format!(
        "{},{}",
        format_length(units, point.x),
        format_length(units, point.y)
    )
}

/// An angle (radians counter-clockwise from east) as the drawing's units
/// write it, counted from the base angle in the angle direction: `45`,
/// `45d30'15"`, `50g`, `0.7854r` or `N 45d E`.
pub fn format_angle(units: &Units, angle: f64) -> String {
    if !angle.is_finite() {
        return angle.to_string();
    }
    let precision = units.angle_precision.min(MAX_PRECISION);
    let p = usize::from(precision);
    // Adding 0.0 turns a negative zero into a plain one.
    let shown = units.to_display_angle(angle).rem_euclid(TAU) + 0.0;
    match units.angle {
        AngleFormat::DecimalDegrees => wrapped(shown.to_degrees(), 360.0, p),
        AngleFormat::DegreesMinutesSeconds => dms(shown.to_degrees(), precision),
        AngleFormat::Gradians => {
            format!("{}g", wrapped(shown.to_degrees() * 400.0 / 360.0, 400.0, p))
        }
        AngleFormat::Radians => format!("{}r", wrapped(shown, TAU, p)),
        AngleFormat::Surveyor => bearing(shown.to_degrees(), precision),
    }
}

/// `-0.0000` is just `0.0000`.
fn unsigned_zero(text: String) -> String {
    match text.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_owned(),
        _ => text,
    }
}

fn scientific(value: f64, p: usize) -> String {
    let text = format!("{value:.p$e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}E{sign}{:02}", exponent.abs())
}

/// Feet and decimal inches: `5'-3.5000"`.
fn engineering(value: f64, p: usize) -> String {
    let scale = 10f64.powi(p as i32);
    let inches_total = (value.abs() * scale).round() / scale;
    let mut feet = (inches_total / 12.0).floor();
    let mut inches = format!("{:.p$}", inches_total - feet * 12.0);
    if inches.parse::<f64>().is_ok_and(|v| v >= 12.0) {
        feet += 1.0;
        inches = format!("{:.p$}", 0.0);
    }
    let sign = if value < 0.0 && inches_total > 0.0 {
        "-"
    } else {
        ""
    };
    format!("{sign}{feet:.0}'-{inches}\"")
}

/// A length counted in steps of 1/2^precision inch (or unit).
struct Steps {
    whole: u64,
    numerator: u64,
    denominator: u64,
}

impl Steps {
    /// `None` when the length is too big to count in steps exactly.
    fn new(length: f64, precision: u8) -> Option<Self> {
        let full = 1_u64 << precision;
        let steps = (length * full as f64).round();
        if !(0.0..1e15).contains(&steps) {
            return None;
        }
        let steps = steps as u64;
        let (whole, remainder) = (steps / full, steps % full);
        let divisor = gcd(remainder, full);
        Some(Self {
            whole,
            numerator: remainder / divisor,
            denominator: full / divisor,
        })
    }

    fn fraction(&self) -> String {
        if self.numerator == 0 {
            String::new()
        } else {
            format!(" {}/{}", self.numerator, self.denominator)
        }
    }

    /// `5'-3 1/2"` (architectural) or `63 1/2` (fractional).
    fn text(&self, negative: bool, feet_and_inches: bool) -> String {
        let nonzero = self.whole > 0 || self.numerator > 0;
        let sign = if negative && nonzero { "-" } else { "" };
        if feet_and_inches {
            format!(
                "{sign}{}'-{}{}\"",
                self.whole / 12,
                self.whole % 12,
                self.fraction()
            )
        } else {
            format!("{sign}{}{}", self.whole, self.fraction())
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// `value` with `p` decimals, written as 0 when rounding reaches a full turn.
fn wrapped(value: f64, turn: f64, p: usize) -> String {
    let scale = 10f64.powi(p as i32);
    let rounded = (value * scale).round() / scale;
    format!("{:.p$}", if rounded >= turn { 0.0 } else { rounded })
}

/// Degrees, minutes and seconds with `precision` choosing the last part
/// shown: 0 degrees, 1 minutes, 2 seconds, then decimals of a second.
fn dms(degrees: f64, precision: u8) -> String {
    let decimals = u32::from(precision.saturating_sub(2));
    let seconds_scale = 10_u64.pow(decimals);
    let per_degree = match precision {
        0 => 1,
        1 => 60,
        _ => 3600 * seconds_scale,
    };
    let counted = (degrees * per_degree as f64).round() as u64;
    let counted = counted % (360 * per_degree);
    match precision {
        0 => format!("{counted}d"),
        1 => format!("{}d{:02}'", counted / 60, counted % 60),
        _ => {
            let (seconds, fraction) = (counted / seconds_scale, counted % seconds_scale);
            let decimals_text = if decimals > 0 {
                format!(".{fraction:0width$}", width = decimals as usize)
            } else {
                String::new()
            };
            format!(
                "{}d{:02}'{:02}{decimals_text}\"",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
        }
    }
}

/// A quadrant bearing (`N 45d E`) for an angle counter-clockwise from east.
fn bearing(degrees: f64, precision: u8) -> String {
    let (from, toward, off_axis) = match degrees {
        d if d <= 90.0 => ("N", "E", 90.0 - d),
        d if d <= 180.0 => ("N", "W", d - 90.0),
        d if d <= 270.0 => ("S", "W", 270.0 - d),
        d => ("S", "E", d - 270.0),
    };
    const EDGE: f64 = 1e-9;
    if off_axis < EDGE {
        from.to_owned()
    } else if off_axis > 90.0 - EDGE {
        toward.to_owned()
    } else {
        format!("{from} {} {toward}", dms(off_axis, precision))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn length(format: LengthFormat, precision: u8) -> Units {
        Units {
            length: format,
            length_precision: precision,
            ..Units::default()
        }
    }

    fn angle(format: AngleFormat, precision: u8) -> Units {
        Units {
            angle: format,
            angle_precision: precision,
            ..Units::default()
        }
    }

    #[test]
    fn decimal_lengths_use_the_precision() {
        let units = length(LengthFormat::Decimal, 4);
        assert_eq!(format_length(&units, 2.0039), "2.0039");
        assert_eq!(format_length(&units, -0.00001), "0.0000");
        assert_eq!(format_length(&length(LengthFormat::Decimal, 0), 12.5), "12");
        assert_eq!(format_length(&units, f64::INFINITY), "inf");
    }

    #[test]
    fn scientific_lengths_have_a_signed_exponent() {
        let units = length(LengthFormat::Scientific, 2);
        assert_eq!(format_length(&units, 15.0), "1.50E+01");
        assert_eq!(format_length(&units, 0.00123), "1.23E-03");
        assert_eq!(format_length(&units, 0.0), "0.00E+00");
    }

    #[test]
    fn engineering_lengths_are_feet_and_decimal_inches() {
        let units = length(LengthFormat::Engineering, 2);
        assert_eq!(format_length(&units, 63.5), "5'-3.50\"");
        assert_eq!(format_length(&units, 3.0), "0'-3.00\"");
        assert_eq!(format_length(&units, -63.5), "-5'-3.50\"");
        assert_eq!(
            format_length(&units, 71.999),
            "6'-0.00\"",
            "carries into the feet"
        );
        assert_eq!(format_length(&units, -0.001), "0'-0.00\"");
    }

    #[test]
    fn architectural_lengths_reduce_their_fractions() {
        let units = length(LengthFormat::Architectural, 4);
        assert_eq!(format_length(&units, 63.5), "5'-3 1/2\"");
        assert_eq!(format_length(&units, 63.25), "5'-3 1/4\"");
        assert_eq!(format_length(&units, 60.0), "5'-0\"");
        assert_eq!(format_length(&units, 0.5), "0'-0 1/2\"");
        assert_eq!(format_length(&units, -63.5), "-5'-3 1/2\"");
        assert_eq!(
            format_length(&length(LengthFormat::Architectural, 0), 63.5),
            "5'-4\""
        );
        assert_eq!(
            format_length(&units, 11.99),
            "1'-0\"",
            "rounds up into the feet"
        );
    }

    #[test]
    fn fractional_lengths_have_no_feet() {
        let units = length(LengthFormat::Fractional, 3);
        assert_eq!(format_length(&units, 3.5), "3 1/2");
        assert_eq!(format_length(&units, 3.0), "3");
        assert_eq!(format_length(&units, 0.125), "0 1/8");
        assert_eq!(format_length(&units, -2.75), "-2 3/4");
        assert_eq!(format_length(&units, 1e20), "100000000000000000000");
    }

    #[test]
    fn decimal_degrees_wrap_at_a_full_turn() {
        let units = angle(AngleFormat::DecimalDegrees, 1);
        assert_eq!(format_angle(&units, FRAC_PI_2), "90.0");
        assert_eq!(format_angle(&units, -FRAC_PI_2), "270.0");
        assert_eq!(format_angle(&units, TAU - 1e-12), "0.0");
        assert_eq!(
            format_angle(&angle(AngleFormat::DecimalDegrees, 0), PI),
            "180"
        );
    }

    #[test]
    fn degrees_minutes_seconds_show_what_the_precision_asks() {
        let a = (45.0 + 30.0 / 60.0 + 15.4 / 3600.0_f64).to_radians();
        assert_eq!(
            format_angle(&angle(AngleFormat::DegreesMinutesSeconds, 0), a),
            "46d"
        );
        assert_eq!(
            format_angle(&angle(AngleFormat::DegreesMinutesSeconds, 1), a),
            "45d30'"
        );
        assert_eq!(
            format_angle(&angle(AngleFormat::DegreesMinutesSeconds, 2), a),
            "45d30'15\""
        );
        assert_eq!(
            format_angle(&angle(AngleFormat::DegreesMinutesSeconds, 3), a),
            "45d30'15.4\""
        );
        let full = (359.0 + 59.0 / 60.0 + 59.9 / 3600.0_f64).to_radians();
        assert_eq!(
            format_angle(&angle(AngleFormat::DegreesMinutesSeconds, 2), full),
            "0d00'00\""
        );
    }

    #[test]
    fn grads_and_radians_carry_a_letter() {
        assert_eq!(
            format_angle(&angle(AngleFormat::Gradians, 2), FRAC_PI_2),
            "100.00g"
        );
        assert_eq!(
            format_angle(&angle(AngleFormat::Radians, 4), 1.0),
            "1.0000r"
        );
    }

    #[test]
    fn surveyor_bearings_count_from_north_or_south() {
        let units = angle(AngleFormat::Surveyor, 0);
        let at = |degrees: f64| format_angle(&units, degrees.to_radians());
        assert_eq!(at(45.0), "N 45d E");
        assert_eq!(at(135.0), "N 45d W");
        assert_eq!(at(225.0), "S 45d W");
        assert_eq!(at(315.0), "S 45d E");
        assert_eq!(
            (at(0.0), at(90.0), at(180.0), at(270.0)),
            ("E".into(), "N".into(), "W".into(), "S".into())
        );
        let minutes = angle(AngleFormat::Surveyor, 1);
        assert_eq!(
            format_angle(&minutes, (90.0_f64 - 30.5).to_radians()),
            "N 30d30' E"
        );
    }

    #[test]
    fn angles_are_counted_from_the_base_angle_in_the_chosen_direction() {
        let units = Units {
            base_angle: FRAC_PI_2,
            clockwise: true,
            ..Units::default()
        };
        assert_eq!(format_angle(&units, FRAC_PI_2), "0");
        assert_eq!(format_angle(&units, 0.0), "90");
        assert_eq!(format_angle(&units, PI), "270");
        let surveyor = Units {
            angle: AngleFormat::Surveyor,
            ..units
        };
        assert_eq!(
            format_angle(&surveyor, FRAC_PI_2),
            "N",
            "bearings are absolute"
        );
    }

    #[test]
    fn points_are_written_x_then_y() {
        let units = length(LengthFormat::Decimal, 1);
        assert_eq!(format_point(&units, Point::new(1.0, -2.25)), "1.0,-2.2");
    }
}
