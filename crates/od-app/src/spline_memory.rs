//! What SPLINE remembers between uses, as AutoCAD's `SPLMETHOD`, `SPLKNOTS`
//! and `SPLDEGREE`: saved with the user's settings, not in the drawing.

use od_commands::{SysVar, Value};
use od_core::KnotParameterization;

use crate::state::AppState;

/// How SPLINE makes the curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Through fit points.
    Fit,
    /// From control vertices.
    Cv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub method: Method,
    pub knots: KnotParameterization,
    /// The CV method's degree, 1 to 5 as `SPLDEGREE` allows.
    pub degree: u8,
}

/// `SPLDEGREE`'s range; the command's Degree option goes up to 10 for one
/// spline without changing it.
pub const DEGREE_RANGE: (u8, u8) = (1, 5);

const DEFAULT: Memory = Memory {
    method: Method::Fit,
    knots: KnotParameterization::Chord,
    degree: 3,
};

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

/// `SPLKNOTS`'s code for a knot spacing.
pub fn knots_code(knots: KnotParameterization) -> i64 {
    match knots {
        KnotParameterization::Chord => 0,
        KnotParameterization::SqrtChord => 1,
        KnotParameterization::Uniform => 2,
    }
}

fn knots_from(code: i64) -> Option<KnotParameterization> {
    match code {
        0 => Some(KnotParameterization::Chord),
        1 => Some(KnotParameterization::SqrtChord),
        2 => Some(KnotParameterization::Uniform),
        _ => None,
    }
}

const KEYS: [&str; 3] = ["spline_method", "spline_knots", "spline_degree"];

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = DEFAULT;
    let Some(storage) = storage else { return m };
    let code = |key: &str| storage.get_string(key).and_then(|v| v.parse::<i64>().ok());
    match code(KEYS[0]) {
        Some(0) => m.method = Method::Fit,
        Some(1) => m.method = Method::Cv,
        _ => {}
    }
    m.knots = code(KEYS[1]).and_then(knots_from).unwrap_or(m.knots);
    m.degree = code(KEYS[2])
        .and_then(|d| u8::try_from(d).ok())
        .filter(|d| (DEGREE_RANGE.0..=DEGREE_RANGE.1).contains(d))
        .unwrap_or(m.degree);
    m
}

/// Write the settings for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    let method = i64::from(m.method == Method::Cv);
    for (key, value) in KEYS
        .iter()
        .zip([method, knots_code(m.knots), i64::from(m.degree)])
    {
        storage.set_string(key, value.to_string());
    }
}

fn int(value: Value) -> i64 {
    match value {
        Value::Int(n) => n,
        _ => 0,
    }
}

/// `SPLMETHOD`, `SPLKNOTS` and `SPLDEGREE`.
pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "SPLMETHOD",
            "How SPLINE makes a spline: 0 through fit points, 1 from control vertices",
            (0, 1),
            |st| Value::Int(i64::from(st.defaults().spline.get().method == Method::Cv)),
            |st, v| {
                let method = if int(v) == 1 { Method::Cv } else { Method::Fit };
                st.defaults().spline.change(|m| m.method = method);
                Ok(())
            },
        ),
        SysVar::int(
            "SPLKNOTS",
            "How SPLINE spaces the knots of a fit spline: 0 chord, 1 square root of chord, 2 uniform",
            (0, 2),
            |st| Value::Int(knots_code(st.defaults().spline.get().knots)),
            |st, v| {
                if let Some(knots) = knots_from(int(v)) {
                    st.defaults().spline.change(|m| m.knots = knots);
                }
                Ok(())
            },
        ),
        SysVar::int(
            "SPLDEGREE",
            "Degree of splines SPLINE makes from control vertices",
            (i64::from(DEGREE_RANGE.0), i64::from(DEGREE_RANGE.1)),
            |st| Value::Int(i64::from(st.defaults().spline.get().degree)),
            |st, v| {
                if let Ok(degree) = u8::try_from(int(v)) {
                    st.defaults().spline.change(|m| m.degree = degree);
                }
                Ok(())
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Saved(std::collections::HashMap<String, String>);

    impl eframe::Storage for Saved {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn the_settings_survive_a_restart_and_bad_saved_values_are_ignored() {
        let kept = Memory {
            method: Method::Cv,
            knots: KnotParameterization::Uniform,
            degree: 5,
        };
        let mut saved = Saved::default();
        save(&kept, &mut saved);
        assert_eq!(load(Some(&saved)), kept);
        use eframe::Storage;
        saved.set_string("spline_degree", "9".into());
        saved.set_string("spline_knots", "7".into());
        let m = load(Some(&saved));
        assert_eq!((m.degree, m.knots), (3, KnotParameterization::Chord));
    }

    #[test]
    fn the_system_variables_read_and_set_the_settings() {
        let table = crate::sysvars::table();
        let mut state = AppState::new();
        table.set("SPLMETHOD", "1", &mut state).unwrap();
        table.set("SPLKNOTS", "1", &mut state).unwrap();
        table.set("SPLDEGREE", "2", &mut state).unwrap();
        let kept = state.defaults().spline.get();
        assert_eq!(kept.method, Method::Cv);
        assert_eq!(kept.knots, KnotParameterization::SqrtChord);
        assert_eq!(table.get("SPLDEGREE", &state), Ok(Value::Int(2)));
        assert!(table.set("SPLDEGREE", "6", &mut state).is_err());
        assert!(table.set("SPLKNOTS", "3", &mut state).is_err());
    }
}
