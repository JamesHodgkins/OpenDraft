//! Where the main window was and whether it was maximised, kept between runs.
//! eframe's own window persistence stores the size of the window as it is at
//! exit, which for a maximised window is the whole desktop, so the size to
//! restore to is lost. This keeps the last normal (not maximised, minimised or
//! fullscreen) rectangle beside the maximised flag. It is a small file next to
//! eframe's storage, because the window is made before the storage is opened.

use std::path::PathBuf;
use std::sync::Mutex;

use eframe::egui;
use serde::{Deserialize, Serialize};

const FILE: &str = "window.json";
/// Smallest size accepted from the file (the window's own minimum).
const MIN_SIZE: [f32; 2] = [640.0, 400.0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowMemory {
    /// Outer top-left corner, in logical pixels.
    pub position: Option<[f32; 2]>,
    /// Inner size, in logical pixels.
    pub size: Option<[f32; 2]>,
    pub maximized: bool,
}

static CURRENT: Mutex<WindowMemory> = Mutex::new(WindowMemory {
    position: None,
    size: None,
    maximized: false,
});

fn path() -> Option<PathBuf> {
    Some(eframe::storage_dir("OpenDraft")?.join(FILE))
}

impl WindowMemory {
    /// The last saved window, or none when there is no usable file.
    pub fn load() -> Self {
        let text = path().and_then(|p| std::fs::read_to_string(p).ok());
        let mut memory: Self = text
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        memory.size = memory.size.filter(|s| {
            s[0] >= MIN_SIZE[0] && s[1] >= MIN_SIZE[1] && s[0] < 20_000.0 && s[1] < 20_000.0
        });
        // A minimised window reports a corner far off screen.
        memory.position = memory
            .position
            .filter(|p| p[0] > -10_000.0 && p[1] > -10_000.0 && p[0] < 20_000.0 && p[1] < 20_000.0);
        *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = memory;
        memory
    }

    /// Apply this to the window about to be made.
    pub fn apply(&self, mut viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
        if let Some(size) = self.size {
            viewport = viewport.with_inner_size(size);
        }
        if let Some(position) = self.position {
            viewport = viewport.with_position(position);
        }
        if self.maximized {
            viewport = viewport.with_maximized(true);
        }
        viewport
    }
}

/// Note the window as it is this frame. The rectangle is only taken while the
/// window is in its normal state, so it stays what a restore returns to.
pub fn observe(ctx: &egui::Context) {
    // The zoom is read outside `input`: egui's lock must not be taken twice.
    let zoom = ctx.zoom_factor();
    let info = ctx.input(|i| i.viewport().clone());
    let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(maximized) = info.maximized {
        current.maximized = maximized;
    }
    let normal =
        !current.maximized && info.minimized != Some(true) && info.fullscreen != Some(true);
    if !normal {
        return;
    }
    // Points include the egui zoom factor; the window wants logical pixels.
    if let Some(inner) = info.inner_rect {
        current.size = Some([inner.width() * zoom, inner.height() * zoom]);
    }
    if let Some(outer) = info.outer_rect {
        current.position = Some([outer.min.x * zoom, outer.min.y * zoom]);
    }
}

/// Write the window to disk; called when the app saves its state.
pub fn save(storage: &mut dyn eframe::Storage) {
    // eframe reads its own saved window at startup even with persist_window
    // off, and it would override ours; blank any entry an earlier build left.
    storage.set_string("window", String::new());
    let current = *CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(&current) {
        let _ = std::fs::write(path, text);
    }
}
