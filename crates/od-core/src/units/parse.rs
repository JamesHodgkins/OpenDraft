//! Reading lengths and angles typed in a drawing's units.

use super::{AngleFormat, Units};

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A typed length, in drawing units. Plain numbers (`12.5`, `1.25e1`), feet and
/// inches (`5'`, `5'-3 1/2"`, `3.5"`) and fractions (`1/2`, `3 1/2`) are all
/// understood whatever the display format, and a bare number is in drawing
/// units, which are inches in feet-and-inches formats.
pub fn parse_length(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some(v) = number(text) {
        return Some(v);
    }
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let total = match body.split_once('\'') {
        Some((feet, rest)) => {
            let rest = rest.trim_start_matches(['-', ' ']);
            let inches = match rest.strip_suffix('"').unwrap_or(rest).trim() {
                "" => 0.0,
                inches => inches_value(inches)?,
            };
            number(feet)? * 12.0 + inches
        }
        None => inches_value(body.strip_suffix('"').unwrap_or(body))?,
    };
    Some(if negative { -total } else { total }).filter(|v| v.is_finite())
}

/// `3`, `3.5`, `1/2`, `3 1/2` or `3-1/2`: a count of inches.
fn inches_value(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some(v) = number(text) {
        return (v >= 0.0).then_some(v);
    }
    let (whole, fraction) = match text.rsplit_once([' ', '-']) {
        Some((whole, fraction)) => (number(whole)?, fraction),
        None => (0.0, text),
    };
    let (top, bottom) = fraction.split_once('/')?;
    let (top, bottom) = (number(top)?, number(bottom)?);
    (whole >= 0.0 && top >= 0.0 && bottom > 0.0).then(|| whole + top / bottom)
}

/// A typed angle, as radians counter-clockwise from east.
///
/// A bare number is in the drawing's angle format (degrees, grads or
/// radians). Letters say what was meant whatever the format: `45d30'15.5"`,
/// `50g`, `0.78r`, and for surveyor's units a bearing such as `N 45d E`. A
/// value is counted from the base angle in the angle direction, except
/// bearings, which are absolute.
pub fn parse_angle(units: &Units, text: &str) -> Option<f64> {
    let text = text.trim();
    if units.angle == AngleFormat::Surveyor {
        if let Some(angle) = bearing(text) {
            return Some(angle.to_radians());
        }
    }
    let shown = if let Some(v) = number(text) {
        match units.angle {
            AngleFormat::Gradians => v * 0.9_f64.to_radians(),
            AngleFormat::Radians => v,
            _ => v.to_radians(),
        }
    } else if let Some(v) = text.strip_suffix(['g', 'G']).and_then(number) {
        (v * 0.9).to_radians()
    } else if let Some(v) = text.strip_suffix(['r', 'R']).and_then(number) {
        v
    } else {
        degrees_minutes_seconds(text)?.to_radians()
    };
    Some(units.from_display_angle(shown)).filter(|v| v.is_finite())
}

/// `45d30'15.5"`: any of the three parts, in order, with `°` for `d`.
fn degrees_minutes_seconds(text: &str) -> Option<f64> {
    let text = text.replace(['\u{b0}', '\u{ba}'], "d");
    let (negative, mut rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text.strip_prefix('+').unwrap_or(&text)),
    };
    let (mut total, mut stage) = (0.0, 0);
    while !rest.is_empty() {
        let end = rest.find(['d', 'D', '\'', '"'])?;
        let (value, tail) = rest.split_at(end);
        let value = number(value)?;
        let mark = tail.chars().next()?;
        match (mark, stage) {
            ('d' | 'D', 0) => (total, stage) = (total + value, 1),
            ('\'', 0 | 1) => (total, stage) = (total + value / 60.0, 2),
            ('"', 0..=2) => (total, stage) = (total + value / 3600.0, 3),
            _ => return None,
        }
        rest = tail[mark.len_utf8()..].trim_start();
    }
    (stage > 0).then_some(if negative { -total } else { total })
}

/// A surveyor's bearing as degrees counter-clockwise from east: `N 45d E`,
/// `S30W`, or just `N`, `E`, `S`, `W`.
fn bearing(text: &str) -> Option<f64> {
    let upper = text.trim().to_ascii_uppercase();
    match upper.as_str() {
        "E" => return Some(0.0),
        "N" => return Some(90.0),
        "W" => return Some(180.0),
        "S" => return Some(270.0),
        _ => {}
    }
    let from = upper.chars().next().filter(|c| matches!(c, 'N' | 'S'))?;
    let toward = upper.chars().last().filter(|c| matches!(c, 'E' | 'W'))?;
    let middle = upper.get(1..upper.len() - 1)?.trim();
    let off_axis = number(middle).map(f64::abs).or_else(|| {
        let lower = middle.to_ascii_lowercase();
        degrees_minutes_seconds(&lower)
    })?;
    if !(0.0..=90.0).contains(&off_axis) {
        return None;
    }
    Some(match (from, toward) {
        ('N', 'E') => 90.0 - off_axis,
        ('N', _) => 90.0 + off_axis,
        (_, 'W') => 270.0 - off_axis,
        _ => 270.0 + off_axis,
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::units::{format_angle, format_length, LengthFormat};

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn plain_numbers_are_drawing_units() {
        assert_eq!(parse_length(" 12.5 "), Some(12.5));
        assert_eq!(parse_length("1.5e1"), Some(15.0));
        assert_eq!(parse_length("-3"), Some(-3.0));
        assert_eq!(parse_length("inf"), None);
        assert_eq!(parse_length("abc"), None);
        assert_eq!(parse_length(""), None);
    }

    #[test]
    fn feet_and_inches_are_twelve_to_the_foot() {
        assert!(close(parse_length("5'"), 60.0));
        assert!(close(parse_length("5'-3 1/2\""), 63.5));
        assert!(close(parse_length("5'3 1/2\""), 63.5));
        assert!(close(parse_length("5' 3.25"), 63.25));
        assert!(close(parse_length("3.5\""), 3.5));
        assert!(close(parse_length("-5'-3\""), -63.0));
        assert!(close(parse_length("0'-0 1/2\""), 0.5));
        assert!(close(parse_length("1.5'"), 18.0));
    }

    #[test]
    fn fractions_are_inches() {
        assert!(close(parse_length("1/2"), 0.5));
        assert!(close(parse_length("3 1/2"), 3.5));
        assert!(close(parse_length("3-1/2"), 3.5));
        assert_eq!(parse_length("1/0"), None);
        assert_eq!(parse_length("'3"), None);
        assert_eq!(parse_length("5'x"), None);
        assert_eq!(parse_length("3\"2\""), None);
    }

    #[test]
    fn written_lengths_read_back() {
        for format in LengthFormat::ALL {
            let units = Units {
                length: format,
                length_precision: 6,
                ..Units::default()
            };
            for value in [0.0, 0.5, 3.75, 63.5, 1000.125, -12.25] {
                let text = format_length(&units, value);
                assert!(close(parse_length(&text), value), "{format:?} {text}");
            }
        }
    }

    #[test]
    fn bare_angles_use_the_angle_format() {
        let degrees = Units::default();
        assert!(close(parse_angle(&degrees, "90"), FRAC_PI_2));
        assert!(close(parse_angle(&degrees, "-30"), -PI / 6.0));
        let grads = Units {
            angle: AngleFormat::Gradians,
            ..degrees
        };
        assert!(close(parse_angle(&grads, "100"), FRAC_PI_2));
        let radians = Units {
            angle: AngleFormat::Radians,
            ..degrees
        };
        assert!(close(parse_angle(&radians, "1.5"), 1.5));
    }

    #[test]
    fn letters_say_the_unit_in_any_format() {
        let units = Units::default();
        assert!(close(parse_angle(&units, "100g"), FRAC_PI_2));
        assert!(close(parse_angle(&units, "1.5r"), 1.5));
        assert!(close(parse_angle(&units, "45d30'"), 45.5_f64.to_radians()));
        assert!(close(
            parse_angle(&units, "45d30'15.5\""),
            (45.0 + 30.0 / 60.0 + 15.5 / 3600.0_f64).to_radians()
        ));
        assert!(close(
            parse_angle(&units, "10\u{b0}"),
            10.0_f64.to_radians()
        ));
        assert!(close(parse_angle(&units, "30'"), 0.5_f64.to_radians()));
        assert!(close(parse_angle(&units, "-5d30'"), -5.5_f64.to_radians()));
        for bad in ["d", "45d30'15\"7", "45'30d", "45dd", "x", "", "1/2"] {
            assert_eq!(parse_angle(&units, bad), None, "{bad}");
        }
    }

    #[test]
    fn bearings_are_absolute() {
        let units = Units {
            angle: AngleFormat::Surveyor,
            base_angle: 1.0,
            clockwise: true,
            ..Units::default()
        };
        assert!(close(parse_angle(&units, "N 45d E"), 45.0_f64.to_radians()));
        assert!(close(parse_angle(&units, "n45w"), 135.0_f64.to_radians()));
        assert!(close(
            parse_angle(&units, "S 45d W"),
            225.0_f64.to_radians()
        ));
        assert!(close(parse_angle(&units, "S45E"), 315.0_f64.to_radians()));
        assert!(close(
            parse_angle(&units, "N 30d30' E"),
            59.5_f64.to_radians()
        ));
        assert!(close(parse_angle(&units, "W"), PI));
        assert_eq!(parse_angle(&units, "N 100d E"), None);
        let degrees = Units::default();
        assert_eq!(
            parse_angle(&degrees, "N 45d E"),
            None,
            "bearings need surveyor's units"
        );
        assert_eq!(parse_angle(&degrees, "E"), None);
    }

    #[test]
    fn typed_angles_count_from_the_base_angle() {
        let units = Units {
            base_angle: FRAC_PI_2,
            ..Units::default()
        };
        assert!(close(parse_angle(&units, "0"), FRAC_PI_2));
        assert!(close(parse_angle(&units, "90"), PI));
        let clockwise = Units {
            clockwise: true,
            ..units
        };
        assert!(close(parse_angle(&clockwise, "90"), 0.0));
    }

    #[test]
    fn written_angles_read_back() {
        for format in AngleFormat::ALL {
            let units = Units {
                angle: format,
                angle_precision: 6,
                base_angle: 0.5,
                clockwise: true,
                ..Units::default()
            };
            for degrees in [10.0_f64, 45.5, 90.0, 200.25, 359.5] {
                let angle = degrees.to_radians();
                let text = format_angle(&units, angle);
                let back = parse_angle(&units, &text).unwrap_or(f64::NAN);
                let diff = (back - angle).rem_euclid(2.0 * PI);
                assert!(diff < 1e-4 || 2.0 * PI - diff < 1e-4, "{format:?} {text}");
            }
        }
    }
}
