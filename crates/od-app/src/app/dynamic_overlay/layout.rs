//! Where the dynamic-input boxes go: arithmetic on rectangles, with nothing
//! painted and no egui context, so the placement rules read in one place.
use eframe::egui::{self, Pos2, Rect};
use od_commands::{DynamicDimensions, Prompt as Ask, Wants};
use od_core::{Point, Units};

use crate::cmdline::Prompt;
use crate::dynamic_input::{avoid, dimensions, DynamicInputState, Field};
use crate::dynamic_settings::DynamicInputSettings;

pub(super) const HEIGHT: f32 = 24.0;
pub(super) const GAP: f32 = 4.0;

/// What the placement depends on besides the input state itself.
#[derive(Clone, Copy)]
pub(super) struct Inputs<'a> {
    pub ask: &'a Ask,
    pub prompt: &'a Prompt,
    pub settings: &'a DynamicInputSettings,
    pub units: &'a Units,
    pub canvas: Rect,
    /// The pointer, on screen.
    pub anchor: Pos2,
    /// The part of the screen around the pointer that stays clear.
    pub clearance: Rect,
    pub show_prompt: bool,
    /// The width of the prompt text; unused when the prompt is not shown.
    pub text_width: f32,
}

/// The boxes to draw and where each goes.
pub(super) struct Plan {
    pub fields: Vec<Field>,
    pub ghost: Option<dimensions::Layout>,
    pub prompt_width: f32,
    pub prompt_rect: Rect,
    /// The row the prompt and the fields sit in (two rows when they wrap).
    pub row: Rect,
    pub rects: Vec<(Field, Rect)>,
}

/// The words of the prompt box: the command, then what it asks.
pub(super) fn prompt_text(prompt: &Prompt) -> String {
    if prompt.command.is_empty() {
        prompt.text.to_string()
    } else {
        format!("{} {}", prompt.command, prompt.text)
    }
}

fn field_width(field: Field) -> f32 {
    if matches!(field, Field::Coordinate | Field::Text | Field::Keyword) {
        144.0
    } else {
        dimensions::BOX.x
    }
}

/// The width of the prompt box for a text `text_width` wide.
fn prompt_box_width(text_width: f32, has_options: bool, canvas: Rect) -> f32 {
    (text_width + if has_options { 38.0 } else { 16.0 })
        .clamp(140.0, 340.0)
        .min((canvas.width() - 16.0).max(1.0))
}

/// A trail of small squares along the line being drawn, which is an obstacle
/// too.
fn rubberband(base: Pos2, end: Pos2) -> Vec<Rect> {
    let steps = (((end - base).length() / 16.0).ceil() as usize).clamp(1, 48);
    (0..=steps)
        .map(|i| {
            let at = base + (end - base) * (i as f32 / steps as f32);
            Rect::from_center_size(at, egui::Vec2::splat(20.0))
        })
        .collect()
}

/// The fields to show: none while a point prompt with the pointer hidden has
/// nothing to say.
fn shown_fields(
    ask: &Ask,
    settings: &DynamicInputSettings,
    input: &DynamicInputState,
) -> Vec<Field> {
    let hidden = ask.wants == Wants::Point
        && settings.pointer_visibility == 0
        && !(settings.dimensions() && ask.dynamic_dimensions != DynamicDimensions::None)
        && !input.editing
        && input.locks.is_empty();
    if hidden {
        Vec::new()
    } else {
        input.fields(ask, settings)
    }
}

/// The dimension lines drawn from the base point to `end`, with the field
/// boxes seated on them, when the fields describe a length or an offset.
fn ghost_dimensions(
    cx: &Inputs,
    fields: &[Field],
    end: Pos2,
    to_screen: &dyn Fn(Point) -> Pos2,
) -> Option<dimensions::Layout> {
    let base = to_screen(cx.ask.base?);
    let relative = !cx.settings.pointer_absolute;
    match fields {
        [Field::Distance, Field::Angle] => dimensions::polar(
            base,
            end,
            -(cx.units.base_angle as f32),
            if cx.units.clockwise { 1.0 } else { -1.0 },
        ),
        [Field::Distance] => dimensions::linear(base, end),
        [Field::X, Field::Y] if relative => dimensions::cartesian(base, end),
        _ => None,
    }
}

/// Place the prompt and the fields. `to_screen` maps a drawing point to the
/// screen. Placement is remembered in `input`, so the boxes keep their side of
/// the pointer from one frame to the next.
pub(super) fn plan(
    cx: &Inputs,
    to_screen: &dyn Fn(Point) -> Pos2,
    input: &mut DynamicInputState,
) -> Plan {
    let Inputs {
        ask,
        prompt,
        settings,
        units,
        canvas,
        anchor,
        clearance,
        show_prompt,
        text_width,
    } = *cx;
    let fields = shown_fields(ask, settings, input);
    let prompt_width = if show_prompt {
        prompt_box_width(text_width, !prompt.options.is_empty(), canvas)
    } else {
        0.0
    };
    // Dimensions follow the point the answer would give, so a typed or locked
    // length shortens them with the line.
    let end = input
        .constrained_point(ask, settings, units)
        .map_or(anchor, to_screen);
    let ghost = ghost_dimensions(cx, &fields, end, to_screen);
    let rubberband = match (&ghost, ask.base) {
        (Some(_), Some(base)) => rubberband(to_screen(base), end),
        _ => Vec::new(),
    };
    // Fields on ghost dimensions are seated there, not in the prompt row.
    let total_fields: f32 = if ghost.is_some() {
        0.0
    } else {
        fields.iter().map(|f| field_width(*f) + GAP).sum()
    };
    let wrap = show_prompt && prompt_width + total_fields > canvas.width() - 16.0;
    let row_size = if wrap {
        egui::vec2(prompt_width.max(total_fields), HEIGHT * 2.0 + GAP)
    } else {
        egui::vec2(prompt_width + total_fields, HEIGHT)
    };
    let row = avoid(
        input.prompt_placement.place(anchor, row_size, canvas),
        canvas,
        &[clearance],
    );
    let prompt_rect = Rect::from_min_size(row.min, egui::vec2(prompt_width, HEIGHT));
    let mut rects: Vec<(Field, Rect)> = Vec::new();
    let mut x = if wrap || !show_prompt {
        row.left()
    } else {
        prompt_rect.right() + GAP
    };
    for field in &fields {
        let seat = ghost
            .as_ref()
            .and_then(|g| g.boxes.iter().find(|b| b.0 == *field));
        let rect = if let Some((_, centre)) = seat {
            let mut obstacles = vec![clearance];
            obstacles.extend(rubberband.iter().copied());
            if show_prompt {
                obstacles.push(prompt_rect.expand(GAP));
            }
            obstacles.extend(rects.iter().map(|(_, r)| r.expand(GAP)));
            avoid(
                Rect::from_center_size(*centre, dimensions::BOX),
                canvas,
                &obstacles,
            )
        } else {
            let rect = Rect::from_min_size(
                egui::pos2(x, row.top() + if wrap { HEIGHT + GAP } else { 0.0 }),
                egui::vec2(field_width(*field), HEIGHT),
            );
            x += field_width(*field) + GAP;
            rect
        };
        rects.push((*field, rect));
    }
    Plan {
        fields,
        ghost,
        prompt_width,
        prompt_rect,
        row,
        rects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(width: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0))
    }

    #[test]
    fn the_prompt_box_is_kept_between_its_limits_and_inside_the_canvas() {
        // Short text still gets the minimum; options add room for the button.
        assert_eq!(prompt_box_width(10.0, false, canvas(1000.0)), 140.0);
        assert_eq!(prompt_box_width(200.0, false, canvas(1000.0)), 216.0);
        assert_eq!(prompt_box_width(200.0, true, canvas(1000.0)), 238.0);
        // Long text stops at the maximum, and a narrow canvas wins over both.
        assert_eq!(prompt_box_width(900.0, true, canvas(1000.0)), 340.0);
        assert_eq!(prompt_box_width(900.0, true, canvas(200.0)), 184.0);
        assert_eq!(prompt_box_width(900.0, true, canvas(10.0)), 1.0);
    }

    #[test]
    fn the_rubberband_is_a_bounded_trail_from_one_end_to_the_other() {
        let base = Pos2::new(10.0, 10.0);
        let short = rubberband(base, Pos2::new(11.0, 10.0));
        assert_eq!(short.len(), 2, "one step, both ends");
        let long = rubberband(base, Pos2::new(5000.0, 10.0));
        assert_eq!(long.len(), 49, "at most 48 steps");
        assert_eq!(long[0].center(), base);
        assert_eq!(long[48].center(), Pos2::new(5000.0, 10.0));
        assert!(long.iter().all(|r| r.size() == egui::vec2(20.0, 20.0)));
    }

    #[test]
    fn text_fields_are_wider_than_number_boxes() {
        assert_eq!(field_width(Field::Coordinate), 144.0);
        assert_eq!(field_width(Field::Keyword), 144.0);
        assert_eq!(field_width(Field::Distance), dimensions::BOX.x);
    }

    #[test]
    fn the_prompt_text_names_the_command_when_there_is_one() {
        let prompt = |command: &'static str| Prompt {
            command: command.into(),
            text: "Specify point:".into(),
            options: Vec::new(),
        };
        assert_eq!(prompt_text(&prompt("")), "Specify point:");
        assert_eq!(prompt_text(&prompt("LINE")), "LINE Specify point:");
    }
}
