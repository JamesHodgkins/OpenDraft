//! Commands, undo/redo and the command registry. No UI dependencies.
//!
//! Every user action (ribbon button, menu item, shortcut, canvas tool) is a
//! named command in a [`Registry`]. Commands that change a drawing do so
//! through an [`Action`] run by the [`Session`], which makes them undoable.

mod action;
mod associations;
mod block_action;
mod changes;
mod context;
mod corner;
mod dim_style_action;
mod dim_styles_action;
mod editor;
mod history;
mod layer_action;
mod layout_action;
mod linetype_action;
mod mleader_style_action;
mod paste_action;
mod point_style_action;
mod prompt;
mod registry;
mod runner;
mod session;
mod sysvar;
mod text_style_action;
mod transaction;
mod units_action;

pub use action::{Action, AddCopies, AddEntity, ModifyAttributes, ModifyEntities, RemoveEntities};
pub use block_action::SetBlocks;
pub use changes::Changes;
pub use context::Context;
pub use dim_style_action::SetDimStyle;
pub use dim_styles_action::SetDimStyles;
pub use editor::{AttributeEditorAnswer, Editor};
pub use history::{History, Provenance, Step, StepInfo};
pub use layer_action::{AddLayer, ModifyLayers, RemoveLayer, SetCurrentLayer, SetNewObjectStyle};
pub use layout_action::SetLayouts;
pub use linetype_action::SetLinetypes;
pub use mleader_style_action::SetMLeaderStyles;
pub use paste_action::{PasteDrawing, WarningSink};
pub use point_style_action::SetPointStyle;
pub use prompt::{
    Answer, BlockDialogButton, Cancelled, DynamicDimensions, Ghost, GhostStyle, Hint, Keyword,
    Marker, NumberOptions, NumberOrPoint, Options, Picked, PointOptions, Prompt, Reply,
    TextOptions, Wants,
};
pub use registry::{
    always, never, AutomationPolicy, Command, CommandEffect, ExecuteError, Registry, RegistryError,
};
pub use runner::{run_scripted, Outcome, Runner, Transcript};
pub use session::{CheckedError, HistoryError, Session};
pub use sysvar::{Kind, SysVar, SysVarError, SysVarTable, TableError, Value};
pub use text_style_action::SetTextStyles;
pub use transaction::Transaction;
pub use units_action::SetUnits;
