//! The drawing's units as the app uses them: reading the active drawing's
//! settings, writing prompt defaults in them, and changing them (the UNITS
//! command and the `LUNITS`... variables).

use od_commands::{Hint, SetUnits, Wants};
use od_core::units::{format_angle, format_length, format_point};
use od_core::{AngleFormat, Units};

use crate::state::AppState;

impl AppState {
    /// The active drawing's units.
    pub fn units(&self) -> Units {
        *self.session().document().units()
    }

    /// Change the active drawing's units as one undo step. Nothing is done
    /// (and nothing recorded) when they are already these.
    pub fn set_units(&mut self, units: Units) {
        let units = units.sanitized();
        if units == self.units() {
            return;
        }
        self.perform(SetUnits::new(units));
        self.message = "Units changed".to_owned();
    }
}

/// An angle for a message such as "Rotated 2 objects 30.00°": the drawing's
/// angle format, with at least two decimals so a typed 30.5 is not reported
/// as 31.
pub fn angle_message(units: &Units, angle: f64) -> String {
    let precise = Units {
        angle_precision: units.angle_precision.max(2),
        ..*units
    };
    let text = format_angle(&precise, angle);
    match units.angle {
        AngleFormat::DecimalDegrees => format!("{text}\u{b0}"),
        _ => text,
    }
}

/// A prompt's default as it is written between `<` and `>`.
pub fn hint_text(units: &Units, wants: Wants, hint: &Hint) -> String {
    match hint {
        Hint::Text(text) => text.clone(),
        Hint::Point(p) => format_point(units, *p),
        Hint::Number(n) => match wants {
            Wants::Distance | Wants::DistanceOrPoint => format_length(units, *n),
            Wants::Angle | Wants::AngleOrPoint => format_angle(units, *n),
            _ => n.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use od_core::{LengthFormat, Point};

    use super::*;

    #[test]
    fn defaults_are_written_in_the_drawings_units() {
        let units = Units {
            length: LengthFormat::Architectural,
            length_precision: 2,
            ..Units::default()
        };
        assert_eq!(
            hint_text(&units, Wants::Distance, &Hint::Number(63.5)),
            "5'-3 1/2\""
        );
        assert_eq!(
            hint_text(&units, Wants::Angle, &Hint::Number(std::f64::consts::PI)),
            "180"
        );
        assert_eq!(hint_text(&units, Wants::Number, &Hint::Number(2.5)), "2.5");
        assert_eq!(
            hint_text(
                &Units::default(),
                Wants::Point,
                &Hint::Point(Point::new(1.0, 2.0))
            ),
            "1.0000,2.0000"
        );
        assert_eq!(
            hint_text(&units, Wants::Keyword, &Hint::Text("Yes".into())),
            "Yes"
        );
    }

    #[test]
    fn messages_keep_two_decimals_of_an_angle() {
        let angle = 30.5_f64.to_radians();
        assert_eq!(angle_message(&Units::default(), angle), "30.50\u{b0}");
        let radians = Units {
            angle: AngleFormat::Radians,
            ..Units::default()
        };
        assert_eq!(angle_message(&radians, 1.0), "1.00r");
    }

    #[test]
    fn changing_units_is_one_undo_step_and_a_no_op_when_unchanged() {
        let mut state = AppState::new();
        let before = state.units();
        state.set_units(before);
        assert!(!state.session().history().can_undo());
        let architectural = Units {
            length: LengthFormat::Architectural,
            ..before
        };
        state.set_units(architectural);
        assert_eq!(state.units(), architectural);
        assert!(state.session().is_modified());
        state.undo();
        assert_eq!(state.units(), before);
    }
}
