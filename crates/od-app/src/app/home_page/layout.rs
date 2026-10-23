//! How wide the Home page is and whether it has a side column.

use eframe::egui;

/// The single-column page's width, and the room kept at each side of it.
const SINGLE_WIDTH: f32 = 720.0;
const SINGLE_MARGIN: f32 = 16.0;
/// From this window width the page has a side column.
const WIDE_FROM: f32 = 900.0;
const WIDE_MARGIN: f32 = 32.0;
/// The most the two columns and the gap between them may take.
const WIDE_MAX: f32 = 1120.0;
const GAP: f32 = 24.0;
const SIDE_MIN: f32 = 280.0;
const SIDE_MAX: f32 = 320.0;
const MAIN_MIN: f32 = 520.0;

/// Where the page's content goes in a window `available` points wide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Layout {
    /// One column `width` wide, `margin` in from the left.
    Single { width: f32, margin: f32 },
    /// A main column, a gap, and a side column.
    Wide { main: f32, side: f32, margin: f32 },
}

pub(super) const COLUMN_GAP: f32 = GAP;

/// Below this width a bar's buttons follow its label, wrapping to the next
/// line, instead of sitting at the right (where they would run over it).
const BAR_WRAPS_BELOW: f32 = 480.0;

/// A line with `label` at the left and `buttons` at the right, as wide as
/// there is room for; narrower, the buttons wrap under the label.
pub(super) fn bar(
    ui: &mut egui::Ui,
    label: impl FnOnce(&mut egui::Ui),
    buttons: impl FnOnce(&mut egui::Ui),
) {
    if ui.available_width() >= BAR_WRAPS_BELOW {
        ui.horizontal(|ui| {
            label(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), buttons);
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            label(ui);
            buttons(ui);
        });
    }
}

pub(super) fn layout(available: f32) -> Layout {
    if available >= WIDE_FROM {
        let content = (available - 2.0 * WIDE_MARGIN).min(WIDE_MAX);
        let side = (content * 0.3).clamp(SIDE_MIN, SIDE_MAX);
        let main = content - side - GAP;
        if main >= MAIN_MIN {
            return Layout::Wide {
                main,
                side,
                margin: (available - content) / 2.0,
            };
        }
    }
    let width = (available - 2.0 * SINGLE_MARGIN).clamp(0.0, SINGLE_WIDTH);
    Layout::Single {
        width,
        margin: (available - width) / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_windows_get_one_column_with_room_at_both_sides() {
        for available in [320.0, 540.0, 800.0, 899.0] {
            let Layout::Single { width, margin } = layout(available) else {
                panic!("one column at {available}");
            };
            assert!(width <= SINGLE_WIDTH);
            assert!(margin >= SINGLE_MARGIN.min(available / 2.0), "{available}");
            assert!((margin * 2.0 + width - available).abs() < 0.01, "centred");
        }
        // 800 x 600 keeps the whole 720 column.
        assert_eq!(
            layout(800.0),
            Layout::Single {
                width: 720.0,
                margin: 40.0
            }
        );
    }

    #[test]
    fn from_900_there_is_a_side_column_that_never_squeezes_the_main_one() {
        for available in [900.0, 1000.0, 1280.0, 1920.0, 3840.0] {
            let Layout::Wide { main, side, margin } = layout(available) else {
                panic!("two columns at {available}");
            };
            assert!(main >= MAIN_MIN, "{available}: {main}");
            assert!((SIDE_MIN..=SIDE_MAX).contains(&side), "{available}: {side}");
            assert!(margin >= WIDE_MARGIN, "{available}");
            assert!(main + side + GAP <= WIDE_MAX + 0.01, "no endless rows");
            assert!((margin * 2.0 + main + side + GAP - available).abs() < 0.01);
        }
    }
}
