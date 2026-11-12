//! A one-shot object snap override: for the next point only, these snaps
//! replace the running ones (AutoCAD's typed `END`, `MID`..., the
//! Shift+right-click menu, and `NON`).

use super::{SnapKind, SnapModes};

/// What replaces the running object snaps for the next point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapOverride {
    /// Only these snaps, even if running snaps are off. If none is found the
    /// point is not placed, and the grid snap is not used either.
    Only(Vec<SnapKind>),
    /// No object snaps at all (`NON`); the grid snap is left as it was.
    None,
}

impl SnapOverride {
    /// `running` with this override in place of its object snaps. Marker,
    /// tooltip and magnet settings are kept.
    pub fn applied(&self, running: &SnapModes) -> SnapModes {
        let mut modes = *running;
        for kind in SnapKind::OBJECT {
            let on = matches!(self, SnapOverride::Only(kinds) if kinds.contains(&kind));
            modes.set(kind, on);
        }
        modes.enabled = true;
        if matches!(self, SnapOverride::Only(_)) {
            modes.grid = false;
        }
        modes
    }

    /// Whether a click that finds no snap is refused instead of placing the
    /// point where the pointer is.
    pub fn insists(&self) -> bool {
        matches!(self, SnapOverride::Only(_))
    }

    /// What the status line names while the override waits: "Endpoint",
    /// "Midpoint, Center", "None".
    pub fn label(&self) -> String {
        match self {
            SnapOverride::None => "None".to_owned(),
            SnapOverride::Only(kinds) => kinds
                .iter()
                .map(|k| k.label())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_replaces_the_running_snaps_even_when_they_are_off() {
        let running = SnapModes {
            enabled: false,
            tooltip: false,
            ..SnapModes::default()
        };
        let modes = SnapOverride::Only(vec![SnapKind::Perpendicular]).applied(&running);
        assert!(modes.enabled && modes.perpendicular);
        assert!(!modes.endpoint && !modes.midpoint && !modes.intersection);
        assert!(!modes.grid, "no silent fall back to the grid");
        assert!(!modes.tooltip, "the display settings stay");
    }

    #[test]
    fn none_turns_every_object_snap_off_and_leaves_the_grid() {
        let modes = SnapOverride::None.applied(&SnapModes::default());
        assert!(SnapKind::OBJECT.iter().all(|k| !modes.allows(*k)));
        assert!(modes.grid && modes.enabled);
        assert!(!SnapOverride::None.insists());
        assert!(SnapOverride::Only(vec![SnapKind::Center]).insists());
    }

    #[test]
    fn the_label_names_each_snap() {
        let o = SnapOverride::Only(vec![SnapKind::Endpoint, SnapKind::Center]);
        assert_eq!(o.label(), "Endpoint, Center");
        assert_eq!(SnapOverride::None.label(), "None");
    }
}
