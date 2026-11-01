//! Numeric property edits for single-line text, including its two-point modes.

use od_core::{arc, Point, SingleLineText, TextAlignmentMode};

use super::Field;

pub(super) fn with_value(
    text: &mut SingleLineText,
    field: Field,
    new: f64,
    positive: bool,
) -> Option<()> {
    match field {
        Field::X1 => {
            if let Some(a) = &mut text.alignment {
                a.second.x += new - text.placement.position.x;
            }
            text.placement.position.x = new;
        }
        Field::Y1 => {
            if let Some(a) = &mut text.alignment {
                a.second.y += new - text.placement.position.y;
            }
            text.placement.position.y = new;
        }
        Field::Angle => set_angle(text, arc::normalize(new.to_radians())),
        Field::Height if positive => set_height(text, new),
        _ => return None,
    }
    Some(())
}

fn set_angle(text: &mut SingleLineText, angle: f64) {
    if let Some(a) = &mut text.alignment {
        let length =
            (a.second.x - text.placement.position.x).hypot(a.second.y - text.placement.position.y);
        a.second = Point::new(
            text.placement.position.x + length * angle.cos(),
            text.placement.position.y + length * angle.sin(),
        );
    }
    text.placement.rotation = angle;
}

fn set_height(text: &mut SingleLineText, height: f64) {
    if let Some(a) = &mut text.alignment {
        match a.mode {
            TextAlignmentMode::Align => {
                let ratio = height / text.placement.height;
                a.second = Point::new(
                    text.placement.position.x + (a.second.x - text.placement.position.x) * ratio,
                    text.placement.position.y + (a.second.y - text.placement.position.y) * ratio,
                );
            }
            TextAlignmentMode::Fit => {
                text.placement.width_factor *= text.placement.height / height;
            }
        }
    }
    text.placement.height = height;
}
