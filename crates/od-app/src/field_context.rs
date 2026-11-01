//! What data fields may know about the world: the active drawing's file name
//! and the time now in the user's own time zone.

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use od_core::block::{FieldContext, FieldTime};

use crate::state::AppState;

/// `time` as the pieces a date pattern picks from.
fn field_time<Tz: TimeZone>(time: &DateTime<Tz>) -> FieldTime {
    FieldTime {
        year: time.year(),
        month: time.month() as u8,
        day: time.day() as u8,
        weekday: time.weekday().num_days_from_monday() as u8,
        hour: time.hour() as u8,
        minute: time.minute() as u8,
        second: time.second() as u8,
    }
}

/// The time now, in the user's own time zone.
pub(crate) fn current_time() -> FieldTime {
    field_time(&Local::now())
}

impl AppState {
    /// The context fields are evaluated in: the active drawing's file name
    /// (none until it is saved) and the time now.
    pub(crate) fn field_context(&self) -> FieldContext {
        FieldContext {
            file_name: self
                .docs
                .get(self.active)
                .and_then(|d| d.path.as_deref())
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            now: Some(current_time()),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn a_time_becomes_its_pieces_with_monday_first() {
        // 2026-11-01 was a Sunday.
        let t = Utc.with_ymd_and_hms(2026, 11, 1, 15, 7, 9).unwrap();
        let f = field_time(&t);
        assert_eq!(
            (f.year, f.month, f.day, f.weekday),
            (2026, 11, 1, 6),
            "Sunday is 6"
        );
        assert_eq!((f.hour, f.minute, f.second), (15, 7, 9));
    }

    #[test]
    fn the_context_names_the_saved_drawing_and_knows_the_time() {
        let mut state = AppState::new();
        assert_eq!(state.field_context().file_name, None);
        let active = state.active;
        state.docs[active].path = Some(std::path::PathBuf::from("c:/work/plan.odx"));
        let context = state.field_context();
        assert_eq!(context.file_name.as_deref(), Some("plan.odx"));
        assert!(context.now.is_some());
    }
}
