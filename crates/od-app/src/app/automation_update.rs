//! Keeping the MCP automation endpoint in step with its setting.

use eframe::egui;

use super::OpenDraftApp;

impl OpenDraftApp {
    /// Start or stop the automation endpoint to match the setting, and answer
    /// the requests waiting for this frame.
    pub(super) fn update_automation(&mut self, ctx: &egui::Context) {
        let wanted = self.state.settings.automation_enabled || self.automation_forced;
        match (wanted, self.automation.is_some()) {
            (true, false) => {
                let waker = {
                    let ctx = ctx.clone();
                    Box::new(move || ctx.request_repaint())
                };
                let started = od_api::instance::directory()
                    .ok_or_else(|| "no per-user runtime folder".to_owned())
                    .and_then(|dir| {
                        crate::automation::AutomationHost::start(&dir, waker)
                            .map_err(|e| e.to_string())
                    });
                match started {
                    Ok(host) => self.automation = Some(host),
                    Err(reason) => {
                        // Do not retry every frame: the user turns it on again.
                        self.state.settings.automation_enabled = false;
                        self.automation_forced = false;
                        self.state.message = format!("Could not start the MCP server: {reason}");
                    }
                }
            }
            (false, true) => {
                self.automation = None;
                // Nobody is left to use an answer.
                self.state.file_ops.cancel_waiting();
            }
            _ => {}
        }
        if let Some(host) = &self.automation {
            host.poll(&mut self.state, &self.registry);
        }
    }
}
