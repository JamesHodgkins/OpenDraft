//! A short memory of how commands ended, so something that started one
//! (an automation connection) can find out later whether it finished or was
//! cancelled. Commands are told apart by `AppState::command_serial`.

use od_commands::Outcome;

use crate::state::AppState;

/// How many endings are remembered.
const KEPT: usize = 32;

impl AppState {
    /// Remember how the current command ended.
    pub(crate) fn note_command_end(&mut self, outcome: Outcome) {
        self.command_ends.push((self.command_serial, outcome));
        if self.command_ends.len() > KEPT {
            self.command_ends.remove(0);
        }
    }

    /// How the command with this serial ended, if it has and is still
    /// remembered.
    pub(crate) fn command_end(&self, serial: u64) -> Option<Outcome> {
        self.command_ends
            .iter()
            .rev()
            .find(|(s, _)| *s == serial)
            .map(|(_, o)| *o)
    }
}
