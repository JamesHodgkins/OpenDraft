//! The in-place multiline text editor. `state` is the draft and every
//! operation on it, with no UI; the rest turns keys and clicks into those
//! operations and paints the result over the drawing.

mod app_state;
mod box_handles;
mod field_dialog;
mod history;
mod input;
mod paragraph_dialog;
mod ruler;
mod state;
mod toolbar;
mod window;
#[cfg(test)]
mod window_tests;

pub use input::Outcome;
pub use state::MTextEditor;
pub use window::show;
