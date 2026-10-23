//! The object snap system variables, with AutoCAD's names and bit values:
//! `OSMODE` (the running snaps), `AUTOSNAP` (marker, tooltip, magnet) and
//! `APBOX` (the aperture box). They mirror `Settings::snap`, so they are
//! saved with the user's settings.

use od_canvas::{SnapKind, SnapModes};
use od_commands::{SysVar, Value};

use crate::settings::Settings;
use crate::state::AppState;

/// `OSMODE` bit of each snap we have.
const OS_BITS: [(SnapKind, i64); 14] = [
    (SnapKind::Endpoint, 1),
    (SnapKind::Midpoint, 2),
    (SnapKind::Center, 4),
    (SnapKind::Node, 8),
    (SnapKind::Quadrant, 16),
    (SnapKind::Intersection, 32),
    (SnapKind::Insertion, 64),
    (SnapKind::Perpendicular, 128),
    (SnapKind::Tangent, 256),
    (SnapKind::Nearest, 512),
    (SnapKind::GeometricCenter, 1024),
    (SnapKind::ApparentIntersection, 2048),
    (SnapKind::Extension, 4096),
    (SnapKind::Parallel, 8192),
];
/// Running object snaps are off (AutoCAD's "suppress" bit).
const OS_OFF: i64 = 16384;
const OS_KNOWN: i64 =
    1 + 2 + 4 + 8 + 16 + 32 + 64 + 128 + 256 + 512 + 1024 + 2048 + 4096 + 8192 + OS_OFF;

/// `AUTOSNAP` bits: marker, tooltip and magnet, then polar tracking, object
/// snap tracking and the tooltips of tracking.
const AS_MARKER: i64 = 1;
const AS_TOOLTIP: i64 = 2;
const AS_MAGNET: i64 = 4;
/// Polar tracking (F10), object snap tracking (F11), and the tooltips of
/// tracking.
const AS_POLAR: i64 = 8;
const AS_OTRACK: i64 = 16;
const AS_TRACK_TIPS: i64 = 32;

/// The `OSMODE` value for these snaps.
pub fn osmode_of(snap: SnapModes) -> i64 {
    let on: i64 = OS_BITS
        .iter()
        .filter(|(kind, _)| snap.allows(*kind))
        .map(|(_, bit)| bit)
        .sum();
    on | if snap.enabled { 0 } else { OS_OFF }
}

/// Apply an `OSMODE` value, leaving the grid snap alone.
pub fn apply_osmode(snap: &mut SnapModes, mode: i64) -> Result<(), String> {
    if mode & !OS_KNOWN != 0 {
        return Err(format!(
            "Only the bits 1 (endpoint), 2 (midpoint), 4 (centre), 8 (node), 16 (quadrant), 32 (intersection), 64 (insertion), 128 (perpendicular), 256 (tangent), 512 (nearest), 1024 (geometric center), 2048 (apparent intersection), 4096 (extension), 8192 (parallel) and {OS_OFF} (off) are supported."
        ));
    }
    for (kind, bit) in OS_BITS {
        snap.set(kind, mode & bit != 0);
    }
    snap.enabled = mode & OS_OFF == 0;
    Ok(())
}

fn autosnap_of(settings: &Settings) -> i64 {
    let (snap, track) = (settings.snap, &settings.tracking);
    let bit = |on: bool, bit: i64| if on { bit } else { 0 };
    bit(snap.marker, AS_MARKER)
        | bit(snap.tooltip, AS_TOOLTIP)
        | bit(snap.magnet, AS_MAGNET)
        | bit(track.polar, AS_POLAR)
        | bit(track.otrack, AS_OTRACK)
        | bit(track.tooltips, AS_TRACK_TIPS)
}

/// `POLARMODE` bits we have: 2 (track along the polar angles), 4 (use the
/// additional angles) and 8 (keep points only with Shift). Bit 1 (measure
/// from the last segment) is not built.
const PM_ALL_POLAR: i64 = 2;
const PM_ADDITIONAL: i64 = 4;
const PM_SHIFT: i64 = 8;

/// The angles of `POLARADDANG`, as AutoCAD takes them: up to ten, separated
/// by semicolons.
fn parse_angles(text: &str) -> Result<Vec<f64>, String> {
    let parts: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() > 10 {
        return Err("At most ten additional angles.".to_owned());
    }
    parts
        .iter()
        .map(|p| {
            p.parse::<f64>()
                .ok()
                .filter(|a| a.is_finite())
                .ok_or_else(|| format!("\"{p}\" is not an angle."))
        })
        .collect()
}

fn int(value: Value) -> i64 {
    match value {
        Value::Int(n) => n,
        _ => 0,
    }
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "OSMODE",
            "Object snaps: 1 endpoint, 2 midpoint, 4 centre, 8 node, 16 quadrant, 32 intersection, 64 insertion, 128 perpendicular, 256 tangent, 512 nearest, 1024 geometric center, 2048 apparent intersection, 4096 extension, 8192 parallel, 16384 off",
            (0, 32767),
            |s| Value::Int(osmode_of(s.settings.snap)),
            |s, v| apply_osmode(&mut s.settings.snap, int(v)),
        ),
        SysVar::int(
            "AUTOSNAP",
            "AutoSnap and AutoTrack: 1 marker, 2 tooltip, 4 magnet, 8 polar tracking, 16 object snap tracking, 32 tracking tooltips",
            (0, 63),
            |s| Value::Int(autosnap_of(&s.settings)),
            |s, v| {
                let bits = int(v);
                s.settings.snap.marker = bits & AS_MARKER != 0;
                s.settings.snap.tooltip = bits & AS_TOOLTIP != 0;
                s.settings.snap.magnet = bits & AS_MAGNET != 0;
                s.settings.tracking.otrack = bits & AS_OTRACK != 0;
                s.settings.tracking.tooltips = bits & AS_TRACK_TIPS != 0;
                s.settings.set_polar(bits & AS_POLAR != 0);
                Ok(())
            },
        ),
        SysVar::real(
            "POLARANG",
            "Polar tracking increment angle in degrees",
            (0.01, 180.0),
            |s| Value::Real(s.settings.tracking.increment),
            |s, v| {
                if let Value::Real(n) = v {
                    s.settings.tracking.increment = n;
                }
                Ok(())
            },
        ),
        SysVar {
            name: "POLARADDANG",
            description: "Additional polar angles in degrees, separated by semicolons",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| {
                let list: Vec<String> = s
                    .settings
                    .tracking
                    .additional
                    .iter()
                    .map(|a| a.to_string())
                    .collect();
                Value::Text(list.join(";"))
            },
            set: |s, v| {
                if let Value::Text(text) = v {
                    s.settings.tracking.additional = parse_angles(&text)?;
                }
                Ok(())
            },
        },
        SysVar::int(
            "POLARMODE",
            "Polar tracking: 2 track along polar angles, 4 additional angles, 8 keep points with Shift",
            (0, 15),
            |s| {
                let t = &s.settings.tracking;
                let bit = |on: bool, bit: i64| if on { bit } else { 0 };
                Value::Int(
                    bit(t.all_polar, PM_ALL_POLAR)
                        | bit(t.use_additional, PM_ADDITIONAL)
                        | bit(t.shift_to_acquire, PM_SHIFT),
                )
            },
            |s, v| {
                let bits = int(v);
                if bits & 1 != 0 {
                    return Err("Measuring from the last segment (bit 1) is not supported.".to_owned());
                }
                let t = &mut s.settings.tracking;
                t.all_polar = bits & PM_ALL_POLAR != 0;
                t.use_additional = bits & PM_ADDITIONAL != 0;
                t.shift_to_acquire = bits & PM_SHIFT != 0;
                Ok(())
            },
        ),
        SysVar::int(
            "TRACKPATH",
            "Tracking path display: 0 all full-screen, 1 polar full-screen, 2 object snap only full-screen, 3 object snap to the cursor",
            (0, 3),
            |s| Value::Int(i64::from(s.settings.tracking.path_display)),
            |s, v| {
                s.settings.tracking.path_display = u8::try_from(int(v)).unwrap_or(0);
                Ok(())
            },
        ),
        SysVar::int(
            "APBOX",
            "Aperture box at the cursor while a point is asked for: 0 off, 1 on",
            (0, 1),
            |s| Value::Int(i64::from(s.settings.snap.aperture_box)),
            |s, v| {
                s.settings.snap.aperture_box = int(v) == 1;
                Ok(())
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osmode_round_trips_through_the_snap_modes() {
        let mut snap = SnapModes::default();
        // Endpoint, midpoint, centre, intersection and extension.
        assert_eq!(osmode_of(snap), 4135);
        apply_osmode(&mut snap, 1).unwrap();
        assert!(snap.enabled && snap.endpoint && !snap.midpoint && !snap.center);
        assert!(snap.grid, "the grid snap is SNAPMODE's");
        apply_osmode(&mut snap, 7 + OS_OFF).unwrap();
        assert!(!snap.enabled && snap.midpoint, "off keeps the chosen modes");
        assert_eq!(osmode_of(snap), 7 + OS_OFF);
        apply_osmode(&mut snap, 4096 + 8192).unwrap();
        assert!(snap.extension && snap.parallel && !snap.endpoint);
    }

    #[test]
    fn every_snap_has_autocads_bit() {
        let mut snap = SnapModes::default();
        apply_osmode(&mut snap, 1 + 16 + 512).unwrap();
        assert!(snap.endpoint && snap.quadrant && snap.nearest && !snap.center);
        assert_eq!(osmode_of(snap), 529);
        apply_osmode(&mut snap, 8 + 32 + 64 + 128 + 256).unwrap();
        assert!(snap.node && snap.intersection && snap.insertion);
        assert!(snap.perpendicular && snap.tangent && !snap.endpoint);
        assert_eq!(osmode_of(snap), 488);
    }

    #[test]
    fn every_osmode_bit_is_a_snap_we_have() {
        let mut snap = SnapModes::default();
        apply_osmode(&mut snap, 1024 + 2048).unwrap();
        assert!(snap.geometric_center && snap.apparent_intersection && !snap.endpoint);
        assert_eq!(osmode_of(snap), 3072);
        // All of AutoCAD's bits are taken; a bit beyond them is not a snap.
        apply_osmode(&mut snap, 16383).unwrap();
        assert!(SnapKind::OBJECT
            .iter()
            .filter(|k| **k != SnapKind::ExtendedIntersection)
            .all(|k| snap.allows(*k)));
        assert!(!snap.extended_intersection, "an override only, never a bit");
        assert!(apply_osmode(&mut snap, 32768).is_err());
    }

    #[test]
    fn autosnap_and_apbox_set_the_feedback() {
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        // Marker, tooltip, magnet, object snap tracking and its tooltips: not
        // polar tracking, which starts off here (AutoCAD's 63 has it on).
        assert_eq!(table.get("AUTOSNAP", &state), Ok(Value::Int(55)));
        table.set("AUTOSNAP", "1", &mut state).unwrap();
        let snap = state.settings.snap;
        assert!(snap.marker && !snap.tooltip && !snap.magnet);
        assert!(!state.settings.tracking.otrack && !state.settings.tracking.polar);
        table.set("AUTOSNAP", "63", &mut state).unwrap();
        assert!(state.settings.tracking.polar && state.settings.tracking.otrack);
        assert!(table.set("AUTOSNAP", "64", &mut state).is_err());
        assert_eq!(table.get("APBOX", &state), Ok(Value::Int(0)));
        table.set("APBOX", "1", &mut state).unwrap();
        assert!(state.settings.snap.aperture_box);
    }

    #[test]
    fn polar_tracking_variables_read_and_write_the_settings() {
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        table.set("POLARANG", "45", &mut state).unwrap();
        assert_eq!(state.settings.tracking.increment, 45.0);
        assert!(table.set("POLARANG", "0", &mut state).is_err());
        assert!(table.set("POLARANG", "200", &mut state).is_err());
        table.set("POLARADDANG", "15;37.5", &mut state).unwrap();
        assert_eq!(state.settings.tracking.additional, [15.0, 37.5]);
        assert_eq!(
            table.get("POLARADDANG", &state),
            Ok(Value::Text("15;37.5".to_owned()))
        );
        assert!(table.set("POLARADDANG", "15;abc", &mut state).is_err());
        table.set("POLARMODE", "14", &mut state).unwrap();
        let t = &state.settings.tracking;
        assert!(t.all_polar && t.use_additional && t.shift_to_acquire);
        assert!(
            table.set("POLARMODE", "1", &mut state).is_err(),
            "relative angles"
        );
        table.set("TRACKPATH", "3", &mut state).unwrap();
        assert_eq!(state.settings.tracking.path_display, 3);
        assert!(table.set("TRACKPATH", "4", &mut state).is_err());
    }

    #[test]
    fn ortho_and_polar_tracking_switch_each_other_off_everywhere() {
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        table.set("ORTHOMODE", "1", &mut state).unwrap();
        table.set("AUTOSNAP", "63", &mut state).unwrap();
        assert!(state.settings.tracking.polar && !state.settings.ortho.enabled);
        table.set("ORTHOMODE", "1", &mut state).unwrap();
        assert!(state.settings.ortho.enabled && !state.settings.tracking.polar);
        assert_eq!(
            table.get("AUTOSNAP", &state).map(|v| v.to_string()),
            Ok("55".to_owned())
        );
    }
}
