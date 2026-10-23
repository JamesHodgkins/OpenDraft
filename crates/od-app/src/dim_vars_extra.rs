//! The dimension style variables for tolerances, alternate units, text fill
//! and fixed extension lines, in the same way as `dim_vars`.

use od_commands::{SysVar, Value};
use od_core::DimStyle;

use crate::dim_vars::{change, flag, int, real, text};
use crate::state::AppState;

/// `DIMTOL` / `DIMLIM` on or off.
pub fn set_tolerance(style: &mut DimStyle, on: bool) {
    match (on, style.tol_mode) {
        (true, 0) => {
            style.tol_mode = if style.tol_plus == style.tol_minus {
                1
            } else {
                2
            }
        }
        (false, 1 | 2) => style.tol_mode = 0,
        _ => {}
    }
}

pub fn set_limits(style: &mut DimStyle, on: bool) {
    match (on, style.tol_mode) {
        (true, _) => style.tol_mode = 3,
        (false, 3) => style.tol_mode = 0,
        _ => {}
    }
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "DIMTOL",
            "Append tolerances to the dimension text: 0 off, 1 on",
            (0, 1),
            |s| flag(matches!(s.dim_style().tol_mode, 1 | 2)),
            |s, v| change(s, |d| set_tolerance(d, int(v) == 1)),
        ),
        SysVar::int(
            "DIMLIM",
            "Show the limits (value plus and minus the tolerance) as the text: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().tol_mode == 3),
            |s, v| change(s, |d| set_limits(d, int(v) == 1)),
        ),
        SysVar::real(
            "DIMTP",
            "Plus tolerance",
            (-1.0e9, 1.0e9),
            |s| Value::Real(s.dim_style().tol_plus),
            |s, v| change(s, |d| d.tol_plus = real(v)),
        ),
        SysVar::real(
            "DIMTM",
            "Minus tolerance",
            (-1.0e9, 1.0e9),
            |s| Value::Real(s.dim_style().tol_minus),
            |s, v| change(s, |d| d.tol_minus = real(v)),
        ),
        SysVar::int(
            "DIMTDEC",
            "Decimal places of the tolerance",
            (0, 8),
            |s| Value::Int(i64::from(s.dim_style().tol_precision)),
            |s, v| change(s, |d| d.tol_precision = int(v) as u8),
        ),
        SysVar::real(
            "DIMTFAC",
            "Height of the tolerance text as a fraction of the dimension text's",
            (1.0e-6, 1.0e3),
            |s| Value::Real(s.dim_style().tol_scale),
            |s, v| change(s, |d| d.tol_scale = real(v)),
        ),
        SysVar::int(
            "DIMALT",
            "Show the measurement in alternate units too: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().alt_on),
            |s, v| change(s, |d| d.alt_on = int(v) == 1),
        ),
        SysVar::real(
            "DIMALTF",
            "Multiplier from the measurement to the alternate units (not zero)",
            (-1.0e6, 1.0e6),
            |s| Value::Real(s.dim_style().alt_factor),
            |s, v| {
                let factor = real(v);
                if factor == 0.0 {
                    return Err("Requires a value other than zero.".to_owned());
                }
                change(s, |d| d.alt_factor = factor)
            },
        ),
        SysVar::int(
            "DIMALTD",
            "Decimal places of the alternate value",
            (0, 8),
            |s| Value::Int(i64::from(s.dim_style().alt_precision)),
            |s, v| change(s, |d| d.alt_precision = int(v) as u8),
        ),
        SysVar {
            name: "DIMAPOST",
            description: "Text after the alternate value; . for none",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| Value::Text(s.dim_style().alt_post),
            set: |s, v| {
                let post = text(v);
                change(s, |d| {
                    d.alt_post = if post == "." { String::new() } else { post }
                })
            },
        },
        SysVar::int(
            "DIMFXLON",
            "Extension lines of one length from the dimension line: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().ext_fixed.is_some()),
            |s, v| {
                let on = int(v) == 1;
                change(s, |d| {
                    d.ext_fixed = if on {
                        Some(d.ext_fixed.unwrap_or(1.0))
                    } else {
                        None
                    }
                })
            },
        ),
        SysVar::real(
            "DIMFXL",
            "Length of fixed extension lines",
            (1.0e-6, 1.0e9),
            |s| Value::Real(s.dim_style().ext_fixed.unwrap_or(1.0)),
            |s, v| change(s, |d| d.ext_fixed = d.ext_fixed.map(|_| real(v))),
        ),
        SysVar::int(
            "DIMTFILL",
            "Background of the dimension text: 0 none, 1 the window's",
            (0, 1),
            |s| flag(s.dim_style().text_fill),
            |s, v| change(s, |d| d.text_fill = int(v) == 1),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::sysvars::table;

    use super::*;

    #[test]
    fn tolerance_variables_set_the_mode_and_the_values() {
        let (table, mut state) = (table(), AppState::new());
        table.set("DIMTP", "0.1", &mut state).unwrap();
        table.set("DIMTM", "0.05", &mut state).unwrap();
        table.set("DIMTOL", "1", &mut state).unwrap();
        assert_eq!(state.dim_style().tol_mode, 2);
        table.set("DIMLIM", "1", &mut state).unwrap();
        assert_eq!(state.dim_style().tol_mode, 3);
        table.set("DIMLIM", "0", &mut state).unwrap();
        assert_eq!(state.dim_style().tol_mode, 0);
        table.set("DIMALT", "1", &mut state).unwrap();
        table.set("DIMALTF", "0.5", &mut state).unwrap();
        assert!(table.set("DIMALTF", "0", &mut state).is_err());
        table.set("DIMFXLON", "1", &mut state).unwrap();
        table.set("DIMFXL", "3", &mut state).unwrap();
        let style = state.dim_style();
        assert!(style.alt_on && style.alt_factor == 0.5);
        assert_eq!(style.ext_fixed, Some(3.0));
        table.set("DIMFXLON", "0", &mut state).unwrap();
        assert_eq!(state.dim_style().ext_fixed, None);
    }
}
