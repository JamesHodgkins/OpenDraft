//! Coordinate interpretation shared by the command line and cursor input.
//! Lengths and angles are read in the drawing's units.
use od_core::units::{parse_angle, parse_length};
use od_core::{Point, Units};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoordinateMode {
    #[default]
    CommandLine,
    DynamicPointer {
        default_relative: bool,
    },
}

pub fn parse_point_with_mode(
    text: &str,
    origin: Point,
    mode: CoordinateMode,
    units: &Units,
) -> Option<Point> {
    let text = text.trim();
    let (relative, body) = if let Some(body) = text.strip_prefix('@') {
        (true, body)
    } else if let Some(body) = text.strip_prefix('#') {
        (false, body)
    } else {
        (
            matches!(
                mode,
                CoordinateMode::DynamicPointer {
                    default_relative: true
                }
            ),
            text,
        )
    };
    let (x, y) = if let Some((a, b)) = body.split_once(',') {
        (parse_length(a)?, parse_length(b)?)
    } else if let Some((d, a)) = body.split_once('<') {
        let (d, a) = (parse_length(d)?, parse_angle(units, a)?);
        (d * a.cos(), d * a.sin())
    } else {
        return None;
    };
    let point = if relative {
        Point::new(origin.x + x, origin.y + y)
    } else {
        Point::new(x, y)
    };
    (point.x.is_finite() && point.y.is_finite()).then_some(point)
}

#[cfg(test)]
mod tests {
    use od_core::{AngleFormat, LengthFormat};

    use super::*;

    #[test]
    fn typed_coordinates_use_the_drawings_units() {
        let units = Units {
            length: LengthFormat::Architectural,
            angle: AngleFormat::Gradians,
            ..Units::default()
        };
        let origin = Point::new(0.0, 0.0);
        let point = |text| parse_point_with_mode(text, origin, CoordinateMode::CommandLine, &units);
        assert_eq!(point("5'-3 1/2\",1'"), Some(Point::new(63.5, 12.0)));
        let polar = point("1'<100g").unwrap();
        assert!(polar.x.abs() < 1e-9 && (polar.y - 12.0).abs() < 1e-9);
        let base = Units {
            base_angle: std::f64::consts::FRAC_PI_2,
            ..Units::default()
        };
        let north =
            parse_point_with_mode("10<0", origin, CoordinateMode::CommandLine, &base).unwrap();
        assert!(
            north.x.abs() < 1e-9 && (north.y - 10.0).abs() < 1e-9,
            "angle 0 points along the base angle"
        );
    }

    #[test]
    fn cursor_defaults_and_overrides_do_not_change_command_line() {
        let base = Point::new(100.0, 200.0);
        let mode = CoordinateMode::DynamicPointer {
            default_relative: true,
        };
        assert_eq!(
            parse_point_with_mode("10,20", base, mode, &Units::default()),
            Some(Point::new(110.0, 220.0))
        );
        for text in ["#10,20", "10,20"] {
            assert_eq!(
                parse_point_with_mode(text, base, CoordinateMode::CommandLine, &Units::default()),
                Some(Point::new(10.0, 20.0))
            );
        }
        assert_eq!(
            parse_point_with_mode("#10,20", base, mode, &Units::default()),
            Some(Point::new(10.0, 20.0))
        );
        assert_eq!(
            parse_point_with_mode(
                "@10,20",
                base,
                CoordinateMode::CommandLine,
                &Units::default()
            ),
            Some(Point::new(110.0, 220.0))
        );
        let point = parse_point_with_mode("10<90", base, mode, &Units::default()).unwrap();
        assert!((point.x - 100.0).abs() < 1e-9 && (point.y - 210.0).abs() < 1e-9);
        for text in ["NaN,0", "inf<0", "1,2,3", "#3", "@#1,2", "1e308,1e308"] {
            let origin = Point::new(1e308, 1e308);
            assert!(
                parse_point_with_mode(text, origin, mode, &Units::default()).is_none(),
                "{text}"
            );
        }
    }
}
