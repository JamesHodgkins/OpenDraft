//! What RECTANGLE remembers between uses: the corner rounding or bevel and
//! the line width, saved with the user's settings. Elevation and thickness are
//! 3D values; this 2D drawing keeps them for the system variables but draws
//! nothing from them.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Memory {
    /// Corner fillet radius; 0 leaves square corners.
    pub radius: f64,
    /// The two chamfer distances; both 0 leaves square corners.
    pub first: f64,
    pub second: f64,
    /// Line width of the rectangle's polyline (`PLINEWID`).
    pub width: f64,
    /// `ELEVATION` and `THICKNESS`.
    pub elevation: f64,
    pub thickness: f64,
}

const KEYS: [&str; 6] = [
    "rectangle_fillet",
    "rectangle_chamfer_first",
    "rectangle_chamfer_second",
    "rectangle_width",
    "rectangle_elevation",
    "rectangle_thickness",
];

/// The saved settings, keeping the default for anything missing or not a
/// sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = Memory::default();
    let Some(storage) = storage else { return m };
    let number = |key: &str| {
        storage
            .get_string(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|n| n.is_finite())
    };
    let size = |key: &str| number(key).filter(|n| *n >= 0.0);
    m.radius = size(KEYS[0]).unwrap_or(m.radius);
    m.first = size(KEYS[1]).unwrap_or(m.first);
    m.second = size(KEYS[2]).unwrap_or(m.second);
    m.width = size(KEYS[3]).unwrap_or(m.width);
    m.elevation = number(KEYS[4]).unwrap_or(m.elevation);
    m.thickness = number(KEYS[5]).unwrap_or(m.thickness);
    m
}

/// Write the settings for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    for (key, value) in KEYS.iter().zip([
        m.radius,
        m.first,
        m.second,
        m.width,
        m.elevation,
        m.thickness,
    ]) {
        storage.set_string(key, value.to_string());
    }
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
        let m = Memory {
            radius: 2.5,
            width: 0.75,
            elevation: -3.0,
            ..Memory::default()
        };
        let mut saved = Saved::default();
        save(&m, &mut saved);
        let m = load(Some(&saved));
        assert_eq!((m.radius, m.width, m.elevation), (2.5, 0.75, -3.0));
        // Nonsense in the saved file leaves the current value alone.
        use eframe::Storage;
        saved.set_string("rectangle_fillet", "not a number".into());
        saved.set_string("rectangle_width", "-4".into());
        let m = load(Some(&saved));
        assert_eq!((m.radius, m.width), (0.0, 0.0));
        assert_eq!(load(None), Memory::default());
    }
}
