//! The unit settings in a DXF header: `$LUNITS`, `$LUPREC`, `$AUNITS`,
//! `$AUPREC`, `$ANGDIR`, `$ANGBASE` and `$INSUNITS`.

use od_core::{AngleFormat, InsertUnits, LengthFormat, Units};

/// The header variables to write, as (name, group code, value). `$INSUNITS` is
/// not among them: R12 has no such variable, so the writer says it was left out.
pub(super) fn header(units: &Units) -> [(&'static str, i32, String); 6] {
    [
        ("$LUNITS", 70, units.length.code().to_string()),
        ("$LUPREC", 70, units.length_precision.to_string()),
        ("$AUNITS", 70, units.angle.code().to_string()),
        ("$AUPREC", 70, units.angle_precision.to_string()),
        ("$ANGDIR", 70, i32::from(units.clockwise).to_string()),
        ("$ANGBASE", 50, units.base_angle.to_degrees().to_string()),
    ]
}

/// The units a header's groups (code, value) set. What is missing or unusable
/// keeps its default.
pub(super) fn read_header<'a>(groups: impl Iterator<Item = (i32, &'a str)>) -> Units {
    let mut units = Units::default();
    let mut variable = "";
    for (code, value) in groups {
        if code == 9 {
            variable = value;
            continue;
        }
        let int = || value.parse::<i64>().ok();
        match variable {
            "$LUNITS" => {
                units.length = int()
                    .and_then(LengthFormat::from_code)
                    .unwrap_or(units.length)
            }
            "$LUPREC" => units.length_precision = precision(value, units.length_precision),
            "$AUNITS" => {
                units.angle = int()
                    .and_then(AngleFormat::from_code)
                    .unwrap_or(units.angle)
            }
            "$AUPREC" => units.angle_precision = precision(value, units.angle_precision),
            "$ANGDIR" => units.clockwise = int() == Some(1),
            "$ANGBASE" => {
                if let Some(degrees) = value.parse::<f64>().ok().filter(|v| v.is_finite()) {
                    units.base_angle = degrees.to_radians();
                }
            }
            "$INSUNITS" => {
                units.insert = int()
                    .and_then(InsertUnits::from_code)
                    .unwrap_or(units.insert)
            }
            _ => {}
        }
        // A variable's value is the one group after its name.
        variable = "";
    }
    units.sanitized()
}

fn precision(value: &str, otherwise: u8) -> u8 {
    value.parse().unwrap_or(otherwise)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_sets_the_units() {
        let groups = [
            (9, "$ACADVER"),
            (1, "AC1027"),
            (9, "$LUNITS"),
            (70, "4"),
            (9, "$LUPREC"),
            (70, "3"),
            (9, "$AUNITS"),
            (70, "1"),
            (9, "$AUPREC"),
            (70, "2"),
            (9, "$ANGDIR"),
            (70, "1"),
            (9, "$ANGBASE"),
            (50, "90.0"),
            (9, "$INSUNITS"),
            (70, "4"),
        ];
        let units = read_header(groups.into_iter());
        assert_eq!(units.length, LengthFormat::Architectural);
        assert_eq!((units.length_precision, units.angle_precision), (3, 2));
        assert_eq!(units.angle, AngleFormat::DegreesMinutesSeconds);
        assert!(units.clockwise);
        assert!((units.base_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(units.insert.code(), 4);
    }

    #[test]
    fn missing_or_bad_values_keep_the_defaults() {
        let groups = [(9, "$LUNITS"), (70, "99"), (9, "$LUPREC"), (70, "x")];
        assert_eq!(read_header(groups.into_iter()), Units::default());
        assert_eq!(read_header(std::iter::empty()), Units::default());
    }

    #[test]
    fn written_variables_read_back() {
        let units = Units {
            length: LengthFormat::Engineering,
            length_precision: 5,
            angle: AngleFormat::Gradians,
            angle_precision: 1,
            clockwise: true,
            base_angle: 1.0,
            ..Units::default()
        };
        let header = header(&units);
        let groups = header
            .iter()
            .flat_map(|(name, code, value)| [(9, name.to_string()), (*code, value.clone())]);
        let groups: Vec<_> = groups.collect();
        let back = read_header(groups.iter().map(|(c, v)| (*c, v.as_str())));
        assert_eq!(back.length, units.length);
        assert_eq!(back.angle, units.angle);
        assert_eq!(back.clockwise, units.clockwise);
        assert!((back.base_angle - 1.0).abs() < 1e-9);
        assert_eq!((back.length_precision, back.angle_precision), (5, 1));
    }
}
