//! What the commands remember between uses (the last rectangle rounding, the
//! last text height...): one value owned by the application and handed to every
//! command it starts, so no command reads hidden global state. The sections
//! are saved with the user's settings.

use std::sync::{Arc, Mutex};

use od_commands::Editor;

use crate::rect_memory;

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
    pub rect: Slot<rect_memory::Memory>,
}

impl CommandDefaults {
    /// The saved sections, keeping the default for anything missing.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        Self {
            rect: Slot::new(rect_memory::load(storage)),
        }
    }

    /// Write the sections for the next run.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        rect_memory::save(&self.rect.get(), storage);
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
