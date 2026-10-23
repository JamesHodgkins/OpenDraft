//! `PDMODE` and `PDSIZE`: how point objects are drawn. They are saved in the
//! drawing, so setting one is an undoable change.

use od_commands::{SetPointStyle, SysVar, Value};
use od_core::PointStyle;

use crate::state::AppState;

impl AppState {
    /// The active drawing's `PDMODE` and `PDSIZE`.
    pub fn point_style(&self) -> PointStyle {
        self.session().document().point_style()
    }

    /// Change the point style as one undo step; nothing is recorded when it
    /// is already this.
    pub fn set_point_style(&mut self, style: PointStyle) {
        if style == self.point_style() {
            return;
        }
        self.perform(SetPointStyle::new(style));
        self.message = format!("PDMODE {} PDSIZE {}", style.mode, style.size);
    }
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "PDMODE",
            "Point look: 0 dot, 1 none, 2 plus, 3 X, 4 tick; add 32 for a circle, 64 for a square",
            (0, 100),
            |s| Value::Int(i64::from(s.point_style().mode)),
            |s, v| {
                let mode = match v {
                    Value::Int(n) => i32::try_from(n).unwrap_or(-1),
                    _ => -1,
                };
                if !PointStyle::valid_mode(mode) {
                    return Err("PDMODE is 0 to 4, plus 32 and 64 for a circle and a square".into());
                }
                let style = s.point_style();
                s.set_point_style(PointStyle { mode, ..style });
                Ok(())
            },
        ),
        SysVar::real(
            "PDSIZE",
            "Point size: above 0 in drawing units, 0 is 5% of the screen height, below 0 a percentage of it",
            (-1e6, 1e6),
            |s| Value::Real(s.point_style().size),
            |s, v| {
                if let Value::Real(size) = v {
                    let style = s.point_style();
                    s.set_point_style(PointStyle { size, ..style });
                }
                Ok(())
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdmode_and_pdsize_are_variables_and_undo() {
        let mut state = AppState::new();
        assert_eq!(state.point_style(), PointStyle::default());
        state.set_point_style(PointStyle {
            mode: 35,
            size: 3.0,
        });
        assert_eq!(state.point_style().mode, 35);
        state.undo();
        assert_eq!(state.point_style(), PointStyle::default());
    }

    #[test]
    fn a_mode_that_is_not_allowed_is_refused() {
        let table = crate::sysvars::table();
        let mut state = AppState::new();
        assert!(table.set("PDMODE", "9", &mut state).is_err());
        assert_eq!(state.point_style().mode, 0);
        table.set("PDMODE", "67", &mut state).unwrap();
        assert_eq!(state.point_style().mode, 67);
    }
}
