//! Picking the plot window on the sheet: the Plot dialog steps aside, the
//! drawing asks for two corners (with a rectangle following the pointer), and
//! the dialog comes back with the window filled in. Escape comes back with
//! nothing changed.

use std::sync::{Arc, Mutex};

use od_commands::{Answer, Cancelled, Editor, Options};
use od_core::{PlotArea, Point};

use crate::state::AppState;

/// Where a finished pick leaves its two corners.
pub type WindowSlot = Arc<Mutex<Option<(Point, Point)>>>;

/// Ask for the two corners of a plot window; a window with no width or no
/// height is told and asked for again.
pub fn ask_window(ed: &Editor) -> Result<(Point, Point), Cancelled> {
    loop {
        let first = ed.get_point(Options::<Point>::new(
            "Specify first corner of the plot window",
        ))?;
        let other = Options::<Point>::new("Specify opposite corner of the plot window");
        let second = match ed.ask_corner(first, other)? {
            Answer::Value(at) => at,
            _ => return Err(Cancelled),
        };
        if first.x != second.x && first.y != second.y {
            return Ok((first, second));
        }
        ed.say("The window has no width or no height.");
    }
}

impl AppState {
    /// Hide the Plot dialog and ask for the plot window on the sheet.
    pub fn plot_dialog_pick_window(&mut self) {
        if self.command_dialogs.plot.is_none() {
            return;
        }
        // The window is in sheet millimetres, not the model a viewport shows.
        if matches!(self.space(), od_canvas::Space::Viewport(..)) {
            self.leave_viewport();
        }
        let slot: WindowSlot = Arc::default();
        if let Some(dialog) = self.command_dialogs.plot.as_mut() {
            dialog.picking = Some(Arc::clone(&slot));
        }
        self.start_command("PLOTWINDOW", move |ed| {
            let window = ask_window(ed)?;
            if let Ok(mut place) = slot.lock() {
                *place = Some(window);
            }
            Ok(())
        });
    }

    /// Bring the Plot dialog back once the pick has ended, with the window if
    /// there was one. Called after the running command is polled, and by the
    /// dialog's own frame in case the command was replaced.
    pub(crate) fn take_window_pick(&mut self) {
        if self.runner.is_some() {
            return;
        }
        let Some(dialog) = self.command_dialogs.plot.as_mut() else {
            return;
        };
        let Some(slot) = dialog.picking.take() else {
            return;
        };
        let window = slot.lock().ok().and_then(|mut place| place.take());
        if let Some((first, second)) = window {
            dialog.draft.plot.area = PlotArea::Window { first, second };
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, PlotArea, Point};

    use crate::state::AppState;

    fn state() -> AppState {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1000.0, 500.0),
        });
        state.new_layout();
        state.open_plot_dialog(false);
        state
    }

    #[test]
    fn the_dialog_steps_aside_and_comes_back_with_the_window() {
        let mut state = state();
        state.plot_dialog_pick_window();
        assert!(state.command_running());
        assert!(!state.modal_open(), "the canvas takes the picks");
        assert!(state
            .command_dialogs
            .plot
            .as_ref()
            .unwrap()
            .picking
            .is_some());
        state.settle_command();
        state.answer_point(Point::new(20.0, 30.0));
        state.settle_command();
        state.answer_point(Point::new(120.0, 90.0));
        state.settle_command();
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert!(dialog.picking.is_none());
        assert_eq!(
            dialog.draft.plot.area,
            PlotArea::Window {
                first: Point::new(20.0, 30.0),
                second: Point::new(120.0, 90.0)
            }
        );
        assert!(state.modal_open());
    }

    #[test]
    fn a_flat_window_is_refused_and_asked_for_again() {
        let mut state = state();
        state.plot_dialog_pick_window();
        state.settle_command();
        state.answer_point(Point::new(20.0, 30.0));
        state.settle_command();
        state.answer_point(Point::new(120.0, 30.0));
        state.settle_command();
        assert!(state.command_running(), "asked for the first corner again");
        assert_eq!(
            state.command_dialogs.plot.as_ref().unwrap().draft.plot.area,
            PlotArea::Layout
        );
        state.cancel_command();
    }

    #[test]
    fn escape_brings_the_dialog_back_unchanged() {
        let mut state = state();
        state.plot_dialog_pick_window();
        state.settle_command();
        state.answer_point(Point::new(20.0, 30.0));
        state.settle_command();
        state.cancel_command();
        state.take_window_pick();
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert!(dialog.picking.is_none());
        assert_eq!(dialog.draft.plot.area, PlotArea::Layout);
        assert!(state.modal_open());
    }

    #[test]
    fn another_command_replacing_the_pick_does_not_leave_the_dialog_hidden() {
        let mut state = state();
        state.plot_dialog_pick_window();
        state.start_command("LINE", |_| Ok(()));
        state.settle_command();
        state.take_window_pick();
        assert!(state
            .command_dialogs
            .plot
            .as_ref()
            .unwrap()
            .picking
            .is_none());
    }
}
