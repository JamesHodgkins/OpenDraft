//! Cursor field state and geometry. Command execution remains in `interact`.
use eframe::egui;
use std::collections::BTreeMap;

use od_commands::{DynamicDimensions, Prompt, Wants};
use od_core::units::{format_angle, format_length, format_point, parse_angle, parse_length};
use od_core::{AngleFormat, Point, Units};

use crate::cmdline::parse_number;
use crate::coordinates::{parse_point_with_mode, CoordinateMode};
use crate::dynamic_settings::DynamicInputSettings;

pub mod dimensions;
mod placement;
pub use placement::{avoid, cursor_clearance, Placement};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    X,
    Y,
    Coordinate,
    Distance,
    Angle,
    Number,
    Text,
    Keyword,
}

/// What was typed into `field`, as the number the field holds: a length in
/// drawing units, an angle in degrees counted from the base angle, or a plain
/// number.
pub fn parse_field(field: Field, text: &str, units: &Units) -> Option<f64> {
    match field {
        Field::Angle => parse_angle(units, text).map(|a| units.to_display_angle(a).to_degrees()),
        Field::Number => parse_number(text),
        _ => parse_length(text),
    }
}

/// A field's number written the way the drawing's units write it.
fn field_text(field: Field, value: f64, units: &Units) -> String {
    match field {
        // Surveyor bearings have no sign; the others show a flipped angle as negative.
        Field::Angle if value < 0.0 && units.angle != AngleFormat::Surveyor => format!(
            "-{}",
            format_angle(units, units.from_display_angle((-value).to_radians()))
        ),
        Field::Angle => format_angle(units, units.from_display_angle(value.to_radians())),
        Field::Number => format!("{value:.4}"),
        _ => format_length(units, value),
    }
}

/// A field's text as drawn when it is not being edited. Only plain degrees
/// need their mark; the other angle formats write their own.
pub fn shown_text(field: Field, live: &str, units: &Units) -> String {
    if field == Field::Angle && units.angle == AngleFormat::DecimalDegrees {
        format!("{live}\u{b0}")
    } else {
        live.to_owned()
    }
}

#[derive(Clone, Default)]
pub struct DynamicInputState {
    pub editing: bool,
    pub active: Option<Field>,
    pub draft: String,
    pub locks: BTreeMap<Field, f64>,
    pub locked_coordinate: Option<Point>,
    pub menu: bool,
    pub selected: usize,
    pub error: Option<String>,
    pub cursor: Option<Point>,
    pub origin: Option<Point>,
    pub canvas: Option<egui::Rect>,
    pub hit_rects: Vec<egui::Rect>,
    pub placements: BTreeMap<Field, Placement>,
    pub prompt_placement: Placement,
    pub signature: String,
    pub request_focus: bool,
}

impl DynamicInputState {
    pub fn clear(&mut self) {
        let (cursor, canvas) = (self.cursor, self.canvas);
        *self = Self::default();
        self.cursor = cursor;
        self.canvas = canvas;
    }

    pub fn fields(&self, ask: &Prompt, settings: &DynamicInputSettings) -> Vec<Field> {
        use Field::*;
        match ask.wants {
            Wants::Point
                if self.draft.contains([',', '<'])
                    || self.draft.starts_with(['#', '@'])
                    || self.locked_coordinate.is_some() =>
            {
                vec![Coordinate]
            }
            Wants::Point
                if ask.base.is_some()
                    && settings.dimensions()
                    && ask.dynamic_dimensions == DynamicDimensions::Polar =>
            {
                vec![Distance, Angle]
            }
            Wants::Point
                if settings.pointer()
                    || (settings.dimensions()
                        && ask.dynamic_dimensions == DynamicDimensions::Cartesian) =>
            {
                if ask.base.is_some()
                    && !settings.pointer_cartesian
                    && ask.dynamic_dimensions != DynamicDimensions::Cartesian
                {
                    vec![Distance, Angle]
                } else {
                    vec![X, Y]
                }
            }
            Wants::Distance if settings.dimensions() => vec![Distance],
            Wants::Angle if settings.dimensions() => vec![Angle],
            Wants::Number => vec![Number],
            Wants::Text => vec![Text],
            _ => Vec::new(),
        }
    }

    pub fn begin(&mut self, field: Field) {
        if self.active != Some(field) {
            self.draft.clear();
        }
        self.active = Some(field);
        self.editing = true;
        self.request_focus = true;
        self.error = None;
    }

    pub fn cycle(
        &mut self,
        fields: &[Field],
        reverse: bool,
        units: &Units,
    ) -> Result<(), &'static str> {
        if fields.is_empty() {
            return Ok(());
        }
        let current = self.active.unwrap_or(fields[0]);
        if matches!(current, Field::Text | Field::Keyword) {
            return Ok(());
        }
        if !self.draft.trim().is_empty() {
            let value = parse_field(current, &self.draft, units)
                .ok_or("Enter a finite numeric value before locking this field.")?;
            self.locks.insert(current, value);
        }
        let index = fields.iter().position(|f| *f == current).unwrap_or(0);
        let next = if reverse {
            (index + fields.len() - 1) % fields.len()
        } else {
            (index + 1) % fields.len()
        };
        self.draft.clear();
        self.begin(fields[next]);
        Ok(())
    }

    pub fn value(
        &self,
        field: Field,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        units: &Units,
    ) -> Option<f64> {
        if self.active == Some(field) && !self.draft.is_empty() {
            if let Some(value) = parse_field(field, &self.draft, units) {
                return Some(value);
            }
        }
        if let Some(value) = self.locks.get(&field) {
            return Some(*value);
        }
        let cursor = self.cursor?;
        let base = ask.base.unwrap_or(Point::new(0.0, 0.0));
        let relative = ask.base.is_some() && !settings.pointer_absolute;
        match field {
            Field::X => Some(cursor.x - if relative { base.x } else { 0.0 }),
            Field::Y => Some(cursor.y - if relative { base.y } else { 0.0 }),
            Field::Distance => Some((cursor.x - base.x).hypot(cursor.y - base.y)),
            Field::Angle => {
                // Half a turn either way, so 270 reads as -90.
                let degrees = units
                    .to_display_angle((cursor.y - base.y).atan2(cursor.x - base.x))
                    .to_degrees()
                    .rem_euclid(360.0);
                Some(if degrees > 180.0 {
                    degrees - 360.0
                } else {
                    degrees
                })
            }
            _ => None,
        }
    }

    /// What the field shows: the typed or locked value, else the cursor's.
    pub fn live_text(
        &self,
        field: Field,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        units: &Units,
    ) -> String {
        self.value(field, ask, settings, units)
            .map(|v| field_text(field, v, units))
            .or_else(|| {
                self.locked_coordinate
                    .filter(|_| field == Field::Coordinate)
                    .map(|p| format_point(units, p))
            })
            .unwrap_or_default()
    }

    pub fn constrained_point(
        &self,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        units: &Units,
    ) -> Option<Point> {
        if self.draft.is_empty() && self.locked_coordinate.is_some() {
            return self.locked_coordinate;
        }
        let active = self.active?;
        if self.locks.is_empty() && self.draft.is_empty() {
            return None;
        }
        if self.draft.contains([',', '<']) || self.draft.starts_with(['#', '@']) {
            let origin = ask.base.or(self.origin).unwrap_or(Point::new(0.0, 0.0));
            return parse_point_with_mode(
                &self.draft,
                origin,
                CoordinateMode::DynamicPointer {
                    default_relative: ask.base.is_some() && !settings.pointer_absolute,
                },
                units,
            );
        }
        if !self.draft.is_empty() && parse_field(active, &self.draft, units).is_none() {
            return None;
        }
        let point = match active {
            Field::X | Field::Y => {
                let origin = if ask.base.is_some() && !settings.pointer_absolute {
                    ask.base?
                } else {
                    Point::new(0.0, 0.0)
                };
                Point::new(
                    origin.x + self.value(Field::X, ask, settings, units)?,
                    origin.y + self.value(Field::Y, ask, settings, units)?,
                )
            }
            Field::Distance | Field::Angle => {
                let base = ask.base?;
                let distance = self.value(Field::Distance, ask, settings, units)?;
                let angle = units.from_display_angle(
                    self.value(Field::Angle, ask, settings, units)?.to_radians(),
                );
                if distance != 0.0
                    && self.cursor == Some(base)
                    && !self.locks.contains_key(&Field::Angle)
                    && active != Field::Angle
                {
                    return None;
                }
                Point::new(
                    base.x + distance * angle.cos(),
                    base.y + distance * angle.sin(),
                )
            }
            _ => return None,
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }

    pub fn submission(
        &self,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        units: &Units,
    ) -> Result<String, &'static str> {
        if ask.wants != Wants::Point && self.draft.is_empty() {
            if let Some(value) = self.active.and_then(|f| self.locks.get(&f)) {
                // A locked angle is in degrees whatever the drawing's angle format.
                return Ok(if ask.wants == Wants::Angle {
                    format!("{value}d")
                } else {
                    value.to_string()
                });
            }
        }
        if let Some(point) = self.locked_coordinate.filter(|_| self.draft.is_empty()) {
            return Ok(format!("#{},{}", point.x, point.y));
        }
        if self.draft.is_empty() && self.locks.is_empty() {
            return Ok(String::new());
        }
        if ask.wants == Wants::Point
            && self
                .active
                .is_some_and(|f| matches!(f, Field::X | Field::Y | Field::Distance | Field::Angle))
            && (self
                .active
                .is_some_and(|f| parse_field(f, &self.draft, units).is_some())
                || (self.draft.is_empty() && !self.locks.is_empty()))
        {
            let point = self
                .constrained_point(ask, settings, units)
                .ok_or("Move the cursor to set a direction first.")?;
            return Ok(format!("#{},{}", point.x, point.y));
        }
        Ok(self.draft.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn line_locks_then_submits_once_through_shared_command_path() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(110.0, 200.0));
        input.begin(Field::Distance);
        input.draft = "10".into();
        input
            .cycle(&[Field::Distance, Field::Angle], false, &Units::default())
            .unwrap();
        input.draft = "90".into();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert_eq!(state.session().document().len(), 1);
        let end = state.docs[0].view.last_point().unwrap();
        assert!((end.x - 100.0).abs() < 1e-9 && (end.y - 210.0).abs() < 1e-9);
        assert!(state.docs[0].dynamic_input.locks.is_empty());
        assert!(state.cmdline.log.last().unwrap().contains("#100,210"));
    }

    #[test]
    fn invalid_submission_preserves_prompt_and_geometry() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        assert!(state.answer_dynamic("NaN,3").is_err());
        assert!(state.command_running());
        assert!(state.session().document().is_empty());
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        state.answer_dynamic("10<90").unwrap();
        state.settle_command();
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn rectangle_fields_use_cartesian_locks_even_with_polar_default() {
        let mut state = AppState::new();
        state.start_command("Rectangle", crate::rectangle::draw_rectangle);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(150.0, 250.0));
        assert_eq!(input.fields(&ask, &settings), [Field::X, Field::Y]);
        input.begin(Field::X);
        input.draft = "10".into();
        input
            .cycle(&[Field::X, Field::Y], false, &Units::default())
            .unwrap();
        input.draft = "20".into();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert_eq!(
            state.docs[0].view.last_point(),
            Some(Point::new(110.0, 220.0))
        );
        assert_eq!(state.session().document().len(), 1);
        assert!(!state.command_running());
    }

    #[test]
    fn tab_locked_scalar_still_submits_to_the_circle_command() {
        let mut state = AppState::new();
        state.start_command("Circle", crate::circle::draw_circle);
        state.settle_command();
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.begin(Field::Distance);
        input.draft = "10".into();
        input
            .cycle(&[Field::Distance], false, &Units::default())
            .unwrap();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert!(matches!(&state.session().document().objects()[0].entity,
            od_core::Entity::Circle { radius, .. } if *radius == 10.0));
    }

    #[test]
    fn undo_first_line_point_does_not_undo_an_existing_object() {
        let mut state = AppState::new();
        state.perform(od_commands::AddEntity::new(od_core::Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        }));
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let option = state.cmd_option_for("U").unwrap();
        state.cmd_choose(option);
        state.settle_command();
        assert!(state.waiting().unwrap().base.is_none());
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn angles_past_half_a_turn_read_as_negative() {
        let units = Units::default();
        assert_eq!(field_text(Field::Angle, -90.0, &units), "-90");
        assert_eq!(parse_field(Field::Angle, "-90", &units), Some(-90.0));
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(0.0, -10.0));
        let value = input.value(Field::Angle, &ask, &settings, &units).unwrap();
        assert!((value + 90.0).abs() < 1e-9);
    }
}
