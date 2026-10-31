//! What HATCH remembers between uses, as AutoCAD's `HPNAME`, `HPANG`,
//! `HPSCALE`, `HPSPACE`, `HPDOUBLE` and `HPISLANDDETECTION`: saved with the
//! user's settings, not in the drawing. The pattern's origin is remembered
//! too (AutoCAD's `HPORIGIN`; it is a point, so it has no variable here).

use od_commands::{Kind, SysVar, Value};
use od_core::hatch::{
    builtin, HatchEntity, HatchFill, HatchLoop, HatchPattern, HatchStyle, PatternType,
};
use od_core::Point;

use crate::state::AppState;

/// What fills the area.
#[derive(Clone, Debug, PartialEq)]
pub enum FillChoice {
    /// A pattern of the catalog, by name (upper case).
    Named(String),
    /// Parallel lines of a given spacing.
    UserDefined,
    Solid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Memory {
    pub fill: FillChoice,
    /// Radians.
    pub angle: f64,
    pub scale: f64,
    pub spacing: f64,
    pub double: bool,
    pub style: HatchStyle,
    pub origin: Point,
    /// Whether a new hatch follows its boundary objects (`HPASSOCIATIVE`).
    pub associative: bool,
}

/// The name `HPNAME` shows for a user-defined pattern, as in AutoCAD.
const USER_NAME: &str = "_USER";

impl Default for Memory {
    fn default() -> Self {
        Self {
            fill: FillChoice::Named("ANSI31".to_owned()),
            angle: 0.0,
            scale: 1.0,
            spacing: 1.0,
            double: false,
            style: HatchStyle::Normal,
            origin: Point::default(),
            associative: true,
        }
    }
}

impl Memory {
    /// The name `HPNAME` reports.
    pub fn name(&self) -> String {
        match &self.fill {
            FillChoice::Named(n) => n.clone(),
            FillChoice::UserDefined => USER_NAME.to_owned(),
            FillChoice::Solid => "SOLID".to_owned(),
        }
    }

    /// Chooses a fill by `HPNAME` text; `None` for a name no pattern has.
    pub fn choice_named(name: &str) -> Option<FillChoice> {
        let upper = name.trim().to_uppercase();
        match upper.as_str() {
            "SOLID" => Some(FillChoice::Solid),
            USER_NAME | "USER" => Some(FillChoice::UserDefined),
            _ => builtin()
                .find(&upper)
                .map(|d| FillChoice::Named(d.name.to_uppercase())),
        }
    }

    /// A hatch of `loops` in the remembered style, fill, angle, scale and
    /// origin, or why there is none.
    pub fn hatch(&self, loops: Vec<HatchLoop>) -> Result<HatchEntity, String> {
        let fill = match &self.fill {
            FillChoice::Solid => HatchFill::Solid,
            FillChoice::UserDefined => HatchFill::Pattern(
                HatchPattern::user_defined(self.spacing, self.angle, self.double, self.origin)
                    .map_err(|e| e.to_string())?,
            ),
            FillChoice::Named(name) => {
                let def = builtin()
                    .find(name)
                    .ok_or_else(|| format!("The pattern {name} is not known."))?;
                HatchFill::Pattern(
                    HatchPattern::from_definition(
                        def,
                        PatternType::Predefined,
                        self.scale,
                        self.angle,
                        self.origin,
                    )
                    .map_err(|e| e.to_string())?,
                )
            }
        };
        let mut hatch = HatchEntity::new(loops, fill);
        hatch.style = self.style;
        Ok(hatch)
    }
}

const KEYS: [&str; 7] = [
    "hatch_name",
    "hatch_angle",
    "hatch_scale",
    "hatch_spacing",
    "hatch_double",
    "hatch_style",
    "hatch_associative",
];

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = Memory::default();
    let Some(storage) = storage else { return m };
    let text = |key: &str| storage.get_string(key);
    let real = |key: &str| {
        text(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    if let Some(choice) = text(KEYS[0]).and_then(|n| Memory::choice_named(&n)) {
        m.fill = choice;
    }
    if let Some(a) = real(KEYS[1]) {
        m.angle = a;
    }
    if let Some(s) = real(KEYS[2]).filter(|s| *s > 0.0) {
        m.scale = s;
    }
    if let Some(s) = real(KEYS[3]).filter(|s| *s > 0.0) {
        m.spacing = s;
    }
    if let Some(d) = real(KEYS[4]) {
        m.double = d != 0.0;
    }
    if let Some(s) = real(KEYS[5]).and_then(|s| HatchStyle::from_code(s as i64)) {
        m.style = s;
    }
    if let Some(a) = real(KEYS[6]) {
        m.associative = a != 0.0;
    }
    m
}

/// Write the settings for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    let values = [
        m.name(),
        m.angle.to_string(),
        m.scale.to_string(),
        m.spacing.to_string(),
        i64::from(m.double).to_string(),
        m.style.code().to_string(),
        i64::from(m.associative).to_string(),
    ];
    for (key, value) in KEYS.iter().zip(values) {
        storage.set_string(key, value);
    }
}

fn real(value: Value) -> f64 {
    match value {
        Value::Real(n) => n,
        Value::Int(n) => n as f64,
        Value::Text(_) => 0.0,
    }
}

/// `HPNAME`, `HPANG`, `HPSCALE`, `HPSPACE`, `HPDOUBLE`, `HPASSOCIATIVE` and
/// `HPISLANDDETECTION`.
pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar {
            name: "HPNAME",
            description: "Default hatch pattern: a pattern name, SOLID or _USER",
            kind: Kind::Text,
            read_only: false,
            get: |st| Value::Text(st.defaults().hatch.get().name()),
            set: |st, v| {
                let Value::Text(name) = v else { return Ok(()) };
                let choice = Memory::choice_named(&name)
                    .ok_or_else(|| format!("The pattern \"{name}\" is not known."))?;
                st.defaults().hatch.change(|m| m.fill = choice);
                Ok(())
            },
        },
        SysVar::real(
            "HPANG",
            "Default hatch pattern angle, in degrees",
            (-360.0, 360.0),
            |st| Value::Real(st.defaults().hatch.get().angle.to_degrees()),
            |st, v| {
                st.defaults()
                    .hatch
                    .change(|m| m.angle = real(v).to_radians());
                Ok(())
            },
        ),
        SysVar::real(
            "HPSCALE",
            "Default hatch pattern scale",
            (1e-9, 1e9),
            |st| Value::Real(st.defaults().hatch.get().scale),
            |st, v| {
                st.defaults().hatch.change(|m| m.scale = real(v));
                Ok(())
            },
        ),
        SysVar::real(
            "HPSPACE",
            "Default spacing of a user-defined hatch pattern",
            (1e-9, 1e9),
            |st| Value::Real(st.defaults().hatch.get().spacing),
            |st, v| {
                st.defaults().hatch.change(|m| m.spacing = real(v));
                Ok(())
            },
        ),
        SysVar::int(
            "HPDOUBLE",
            "Whether a user-defined hatch pattern is doubled: 0 single, 1 double",
            (0, 1),
            |st| Value::Int(i64::from(st.defaults().hatch.get().double)),
            |st, v| {
                st.defaults().hatch.change(|m| m.double = real(v) != 0.0);
                Ok(())
            },
        ),
        SysVar::int(
            "HPASSOCIATIVE",
            "Whether a new hatch follows its boundary objects: 0 no, 1 yes",
            (0, 1),
            |st| Value::Int(i64::from(st.defaults().hatch.get().associative)),
            |st, v| {
                st.defaults()
                    .hatch
                    .change(|m| m.associative = real(v) != 0.0);
                Ok(())
            },
        ),
        SysVar::int(
            "HPISLANDDETECTION",
            "How hatch treats islands: 0 normal, 1 outer, 2 ignore",
            (0, 2),
            |st| Value::Int(st.defaults().hatch.get().style.code()),
            |st, v| {
                if let Some(style) = HatchStyle::from_code(real(v) as i64) {
                    st.defaults().hatch.change(|m| m.style = style);
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
        let before = Memory {
            fill: FillChoice::Named("ANSI32".into()),
            angle: 0.5,
            scale: 3.0,
            spacing: 2.5,
            double: true,
            style: HatchStyle::Ignore,
            ..Memory::default()
        };
        let mut saved = Saved::default();
        save(&before, &mut saved);
        assert_eq!(load(Some(&saved)), before);
        use eframe::Storage;
        saved.set_string("hatch_name", "NOSUCH".into());
        saved.set_string("hatch_scale", "-4".into());
        saved.set_string("hatch_style", "9".into());
        saved.set_string("hatch_angle", "nan".into());
        let m = load(Some(&saved));
        assert_eq!(m.fill, FillChoice::Named("ANSI31".into()));
        assert_eq!((m.scale, m.style, m.angle), (1.0, HatchStyle::Normal, 0.0));
    }

    #[test]
    fn the_system_variables_read_and_set_the_settings() {
        let table = crate::sysvars::table();
        let mut state = AppState::new();
        table.set("HPNAME", "ansi32", &mut state).unwrap();
        table.set("HPANG", "45", &mut state).unwrap();
        table.set("HPSCALE", "2.5", &mut state).unwrap();
        table.set("HPDOUBLE", "1", &mut state).unwrap();
        table.set("HPISLANDDETECTION", "1", &mut state).unwrap();
        let m = state.defaults().hatch.get();
        assert_eq!(m.fill, FillChoice::Named("ANSI32".into()));
        assert!((m.angle - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert_eq!((m.scale, m.double, m.style), (2.5, true, HatchStyle::Outer));
        assert_eq!(
            table.get("HPNAME", &state),
            Ok(Value::Text("ANSI32".into()))
        );
        assert!(table.set("HPNAME", "nosuch", &mut state).is_err());
        assert!(table.set("HPSCALE", "0", &mut state).is_err());
        assert!(table.set("HPISLANDDETECTION", "3", &mut state).is_err());
        table.set("HPNAME", "solid", &mut state).unwrap();
        assert_eq!(state.defaults().hatch.get().fill, FillChoice::Solid);
        table.set("HPNAME", "_user", &mut state).unwrap();
        assert_eq!(state.defaults().hatch.get().fill, FillChoice::UserDefined);
    }

    #[test]
    fn a_hatch_is_made_in_the_remembered_style() {
        let mut m = Memory {
            scale: 10.0,
            angle: std::f64::consts::FRAC_PI_4,
            style: HatchStyle::Outer,
            ..Memory::default()
        };
        let h = m.hatch(Vec::new()).unwrap();
        assert_eq!(h.style, HatchStyle::Outer);
        let HatchFill::Pattern(p) = &h.fill else {
            panic!("pattern");
        };
        // The same numbers AutoCAD stored for ANSI31 at scale 10 and 45 degrees.
        assert!((p.lines[0].offset[0] + 1.25).abs() < 1e-12);
        m.fill = FillChoice::Solid;
        assert_eq!(m.hatch(Vec::new()).unwrap().fill, HatchFill::Solid);
        m.fill = FillChoice::Named("NOSUCH".into());
        assert!(m.hatch(Vec::new()).is_err());
    }
}
