//! The unit system variables, with AutoCAD's names and values. They are saved
//! in the drawing, so setting one is an undoable change (like the UNITS dialog).

use od_commands::{SysVar, Value};
use od_core::{AngleFormat, InsertUnits, LengthFormat, Units};

use crate::state::AppState;

fn int(value: Value) -> i64 {
    match value {
        Value::Int(n) => n,
        _ => 0,
    }
}

fn change(s: &mut AppState, edit: impl FnOnce(&mut Units)) -> Result<(), String> {
    let mut units = s.units();
    edit(&mut units);
    s.set_units(units);
    Ok(())
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "LUNITS",
            "Length format: 1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional",
            (1, 5),
            |s| Value::Int(s.units().length.code()),
            |s, v| {
                let format = LengthFormat::from_code(int(v)).unwrap_or_default();
                change(s, |u| u.length = format)
            },
        ),
        SysVar::int(
            "LUPREC",
            "Length precision: decimal places, or 2 to the power of this as the fraction step",
            (0, 8),
            |s| Value::Int(i64::from(s.units().length_precision)),
            |s, v| change(s, |u| u.length_precision = int(v) as u8),
        ),
        SysVar::int(
            "AUNITS",
            "Angle format: 0 decimal degrees, 1 deg/min/sec, 2 grads, 3 radians, 4 surveyor's",
            (0, 4),
            |s| Value::Int(s.units().angle.code()),
            |s, v| {
                let format = AngleFormat::from_code(int(v)).unwrap_or_default();
                change(s, |u| u.angle = format)
            },
        ),
        SysVar::int(
            "AUPREC",
            "Angle precision: decimal places",
            (0, 8),
            |s| Value::Int(i64::from(s.units().angle_precision)),
            |s, v| change(s, |u| u.angle_precision = int(v) as u8),
        ),
        SysVar::int(
            "ANGDIR",
            "Positive angles: 0 counter-clockwise, 1 clockwise",
            (0, 1),
            |s| Value::Int(i64::from(s.units().clockwise)),
            |s, v| change(s, |u| u.clockwise = int(v) == 1),
        ),
        SysVar::real(
            "ANGBASE",
            "Direction of angle 0, in degrees counter-clockwise from east",
            (0.0, 360.0),
            |s| Value::Real(s.units().base_angle.to_degrees()),
            |s, v| {
                if let Value::Real(degrees) = v {
                    change(s, |u| u.base_angle = degrees.to_radians())?;
                }
                Ok(())
            },
        ),
        SysVar::int(
            "INSUNITS",
            "Units of inserted content: 0 unitless, 1 inches, 2 feet, 4 millimeters, 6 meters...",
            (0, 20),
            |s| Value::Int(s.units().insert.code()),
            |s, v| {
                let insert = InsertUnits::from_code(int(v)).unwrap_or_default();
                change(s, |u| u.insert = insert)
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::sysvars::table;

    use super::*;

    #[test]
    fn the_variables_read_and_set_the_drawings_units() {
        let (table, mut state) = (table(), AppState::new());
        assert_eq!(table.get("LUNITS", &state), Ok(Value::Int(2)));
        assert_eq!(table.get("LUPREC", &state), Ok(Value::Int(4)));
        assert_eq!(table.get("AUNITS", &state), Ok(Value::Int(0)));
        table.set("lunits", "4", &mut state).unwrap();
        table.set("LUPREC", "3", &mut state).unwrap();
        table.set("AUNITS", "1", &mut state).unwrap();
        table.set("AUPREC", "2", &mut state).unwrap();
        table.set("ANGDIR", "1", &mut state).unwrap();
        table.set("ANGBASE", "90", &mut state).unwrap();
        table.set("INSUNITS", "1", &mut state).unwrap();
        let units = state.units();
        assert_eq!(units.length, LengthFormat::Architectural);
        assert_eq!((units.length_precision, units.angle_precision), (3, 2));
        assert_eq!(units.angle, AngleFormat::DegreesMinutesSeconds);
        assert!(units.clockwise);
        assert!((units.base_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(table.get("ANGBASE", &state), Ok(Value::Real(90.0)));
        assert_eq!(units.insert.code(), 1);
        assert!(
            state.session().is_modified(),
            "they are saved in the drawing"
        );
        for bad in [
            ("LUNITS", "6"),
            ("LUPREC", "9"),
            ("ANGDIR", "2"),
            ("INSUNITS", "21"),
        ] {
            assert!(table.set(bad.0, bad.1, &mut state).is_err(), "{bad:?}");
        }
        assert_eq!(state.units(), units);
    }
}
