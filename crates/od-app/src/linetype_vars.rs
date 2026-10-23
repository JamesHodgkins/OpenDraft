//! The linetype system variables. `LTSCALE` is saved in the drawing, so
//! setting it is an undoable change.

use od_commands::{SetLinetypes, SysVar, Value};

use crate::state::AppState;

impl AppState {
    /// The active drawing's `LTSCALE`.
    pub fn linetype_scale(&self) -> f64 {
        self.session().document().linetype_scale()
    }

    /// Change `LTSCALE` as one undo step; nothing is recorded when it is
    /// already this.
    pub fn set_linetype_scale(&mut self, scale: f64) {
        if scale == self.linetype_scale() {
            return;
        }
        let table = self.session().document().linetypes().clone();
        self.set_linetypes(table, scale);
        self.message = format!("Linetype scale {scale}");
    }

    /// Replace the linetype table and `LTSCALE` as one undo step (the
    /// Linetype Manager's OK); nothing is recorded when both are as they were.
    pub fn set_linetypes(&mut self, table: od_core::LinetypeTable, scale: f64) {
        let document = &self.session().document();
        if table == *document.linetypes() && scale == document.linetype_scale() {
            return;
        }
        self.perform(SetLinetypes::new(table, scale));
        self.message = "Linetypes changed".to_owned();
    }
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![SysVar::real(
        "LTSCALE",
        "Global linetype scale: every pattern length is multiplied by it",
        (1e-6, 1e6),
        |s| Value::Real(s.linetype_scale()),
        |s, v| {
            if let Value::Real(scale) = v {
                s.set_linetype_scale(scale);
            }
            Ok(())
        },
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ltscale_is_a_variable_and_undoes() {
        let mut state = AppState::new();
        assert_eq!(state.linetype_scale(), 1.0);
        state.set_linetype_scale(2.0);
        assert_eq!(state.linetype_scale(), 2.0);
        state.undo();
        assert_eq!(state.linetype_scale(), 1.0);
    }
}
