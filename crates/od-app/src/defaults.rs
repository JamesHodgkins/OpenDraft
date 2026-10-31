//! What the commands remember between uses (the last rectangle rounding, the
//! last text height...): one value owned by the application and handed to every
//! command it starts, so no command reads hidden global state. The sections
//! are saved with the user's settings.

use std::sync::{Arc, Mutex};

use od_commands::Editor;

use crate::{
    corner_memory, overkill_memory, pedit_memory, purge_memory, rect_memory, spline_memory,
    text_memory,
};

/// One remembered section, shared between the application and the commands
/// running on their own threads.
#[derive(Default)]
pub struct Slot<T>(Mutex<T>);

impl<T: Clone> Slot<T> {
    pub fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    /// The section as it is now.
    pub fn get(&self) -> T {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Change the section; used by a command's options, the system variables
    /// and loading the user's settings.
    pub fn change<R>(&self, change: impl FnOnce(&mut T) -> R) -> R {
        change(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[derive(Default)]
pub struct CommandDefaults {
    pub corner: Slot<corner_memory::Memory>,
    pub overkill: Slot<od_core::overkill::Settings>,
    pub pedit: Slot<pedit_memory::Memory>,
    pub purge: Slot<purge_memory::Memory>,
    pub rect: Slot<rect_memory::Memory>,
    pub spline: Slot<spline_memory::Memory>,
    pub text: Slot<text_memory::Memory>,
}

impl CommandDefaults {
    /// The saved sections, keeping the default for anything missing.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        Self {
            corner: Slot::new(corner_memory::load(storage)),
            overkill: Slot::new(overkill_memory::load(storage)),
            pedit: Slot::new(pedit_memory::load(storage)),
            purge: Slot::new(purge_memory::load(storage)),
            rect: Slot::new(rect_memory::load(storage)),
            spline: Slot::new(spline_memory::load(storage)),
            text: Slot::new(text_memory::load(storage)),
        }
    }

    /// Write the sections for the next run.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        corner_memory::save(&self.corner.get(), storage);
        overkill_memory::save(&self.overkill.get(), storage);
        pedit_memory::save(&self.pedit.get(), storage);
        purge_memory::save(&self.purge.get(), storage);
        rect_memory::save(&self.rect.get(), storage);
        spline_memory::save(&self.spline.get(), storage);
        text_memory::save(&self.text.get(), storage);
    }
}

/// How a command reaches its defaults.
pub trait EditorDefaults {
    /// The application's defaults. A command started without them (a test of
    /// the command library alone) gets a fresh set that nothing else sees.
    fn defaults(&self) -> Arc<CommandDefaults>;
}

impl EditorDefaults for Editor {
    fn defaults(&self) -> Arc<CommandDefaults> {
        self.context().get().unwrap_or_default()
    }
}
