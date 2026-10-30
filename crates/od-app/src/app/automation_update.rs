//! Keeping the MCP automation endpoint in step with its setting.

use std::sync::{Arc, OnceLock};

use eframe::egui;

use super::OpenDraftApp;

/// The core every connection goes through, the socket in front of it while
/// the setting is on, and the handle that lets the core's waker repaint the
/// window once there is one (the core is made first).
pub(super) struct Automation {
    pub core: Arc<crate::automation::AutomationCore>,
    pub socket: Option<crate::automation::SocketServer>,
    repaint: Arc<OnceLock<egui::Context>>,
    /// The chat panel's conversations; it shares the core.
    pub assistant: crate::assistant::Assistant,
}

impl Automation {
    pub fn new() -> Self {
        let repaint: Arc<OnceLock<egui::Context>> = Arc::default();
        let core = crate::automation::AutomationCore::new(
            crate::automation::fresh_instance_id(),
            Box::new({
                let repaint = Arc::clone(&repaint);
                move || {
                    if let Some(ctx) = repaint.get() {
                        ctx.request_repaint();
                    }
                }
            }),
        );
        // The key may come from the environment; it is otherwise typed into
        // the panel and kept for the session only.
        let key = std::env::var("MISTRAL_API_KEY").ok();
        let wake: crate::assistant::Waker = Arc::new({
            let repaint = Arc::clone(&repaint);
            move || {
                if let Some(ctx) = repaint.get() {
                    ctx.request_repaint();
                }
            }
        });
        Self {
            core: Arc::new(core),
            socket: None,
            repaint,
            assistant: crate::assistant::Assistant::new(
                crate::assistant::mistral(),
                wake,
                key,
                crate::assistant::os_key_store(),
            ),
        }
    }
}

impl OpenDraftApp {
    /// Start or stop the automation endpoint to match the setting, and answer
    /// the requests waiting for this frame.
    pub(super) fn update_automation(&mut self, ctx: &egui::Context) {
        let _ = self.automation.repaint.set(ctx.clone());
        let wanted = self.state.settings.automation_enabled || self.automation_forced;
        match (wanted, self.automation.socket.is_some()) {
            (true, false) => {
                let started = od_api::instance::directory()
                    .ok_or_else(|| "no per-user runtime folder".to_owned())
                    .and_then(|dir| {
                        crate::automation::SocketServer::start(&dir, &self.automation.core)
                            .map_err(|e| e.to_string())
                    });
                match started {
                    Ok(server) => self.automation.socket = Some(server),
                    Err(reason) => {
                        // Do not retry every frame: the user turns it on again.
                        self.state.settings.automation_enabled = false;
                        self.automation_forced = false;
                        self.state.message = format!("Could not start the MCP server: {reason}");
                    }
                }
            }
            (false, true) => {
                // The outside programs go; the in-app assistant stays.
                let outside: Vec<u64> = self
                    .automation
                    .core
                    .grants()
                    .into_iter()
                    .filter(|(_, g)| !g.in_process)
                    .map(|(id, _)| id)
                    .collect();
                self.automation.socket = None;
                // Nobody is left to use an answer.
                self.state.file_ops.cancel_waiting();
                crate::automation::withdraw_selection_requests(
                    &mut self.state,
                    |grant| outside.contains(&grant),
                    "Automation was turned off, so the request to select objects was dropped.",
                );
            }
            _ => {}
        }
        self.automation.assistant.poll();
        // Nothing to answer and nobody to answer it: leave the frame alone.
        if self.automation.socket.is_some() || !self.automation.core.grants().is_empty() {
            self.automation.core.poll(&mut self.state, &self.registry);
        }
    }
}
