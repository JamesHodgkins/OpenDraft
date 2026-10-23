//! OpenDraft UI toolkit: theme and widgets built on egui.

mod doc_tabs;
pub mod icon_set;
pub mod icons;
pub mod menu;
pub mod ribbon;
pub mod shortcuts;
mod theme;
pub mod widgets;

pub use doc_tabs::{document_tabs, DocumentTabEvent};
pub use theme::{icon_family, Colors, Palette, Theme};
