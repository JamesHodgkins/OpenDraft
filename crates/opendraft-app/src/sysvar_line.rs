//! System variables at the command line: `SETVAR`, or just the variable's
//! name, as in AutoCAD. Typing a name shows the value and, for a variable
//! that can be set, asks for a new one (Enter keeps the old one).
//!
//! This is a small two-step prompt of its own rather than an `Editor`
//! command: it works on the application's settings, which a command thread
//! cannot reach, and needs no undo.

use std::borrow::Cow;

use crate::cmdline::Prompt;
use crate::state::AppState;
use crate::sysvars::table;

/// What the command line is waiting for after `SETVAR` or a variable name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysVarPrompt {
    /// `SETVAR`: the name of a variable, or `?`.
    Name,
    /// A new value for this variable.
    Value(&'static str),
}

impl AppState {
    /// The `SETVAR` command.
    pub fn begin_setvar(&mut self) {
        self.abort_command();
        self.cmdline.echo_command("SETVAR");
        self.cmdline.sysvar = Some(SysVarPrompt::Name);
    }

    /// Forget a half-finished variable prompt (Esc, or another command).
    pub(crate) fn end_sysvar_prompt(&mut self) -> bool {
        self.cmdline.sysvar.take().is_some()
    }

    pub(crate) fn sysvar_prompt(&self) -> Option<Prompt> {
        let (text, command): (Cow<'static, str>, &'static str) =
            match self.cmdline.sysvar.as_ref()? {
                SysVarPrompt::Name => ("Enter variable name or [?]".into(), "SETVAR"),
                SysVarPrompt::Value(name) => {
                    let current = table()
                        .get(name, self)
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    (format!("Enter new value for {name} <{current}>").into(), "")
                }
            };
        Some(Prompt {
            command: command.into(),
            text,
            options: Vec::new(),
        })
    }

    /// Take a typed line for the variable prompt, if there is one.
    /// Returns whether it was used.
    pub(crate) fn sysvar_submit(&mut self, text: &str) -> bool {
        let Some(waiting) = self.cmdline.sysvar.clone() else {
            return false;
        };
        let text = text.trim();
        let asked = self.sysvar_prompt().map(|p| p.plain()).unwrap_or_default();
        self.cmdline.say(format!("{asked} {text}"));
        match waiting {
            SysVarPrompt::Name => self.sysvar_name(text),
            SysVarPrompt::Value(name) => {
                if text.is_empty() {
                    self.cmdline.sysvar = None;
                    return true;
                }
                match table().set(name, text, self) {
                    Ok(value) => {
                        self.cmdline.say(format!("{name} = {value}"));
                        self.cmdline.sysvar = None;
                    }
                    // Ask again, as AutoCAD does.
                    Err(why) => self.cmdline.say(why.to_string()),
                }
            }
        }
        true
    }

    /// A variable name typed at the command line with no command running.
    /// Returns whether `text` named one.
    pub(crate) fn sysvar_typed(&mut self, text: &str) -> bool {
        if self.command_running() || table().find(text).is_none() {
            return false;
        }
        self.sysvar_name(text);
        true
    }

    fn sysvar_name(&mut self, text: &str) {
        let table = table();
        if text.is_empty() {
            self.cmdline.sysvar = None;
        } else if let Some(pattern) = text.strip_prefix('?') {
            let pattern = if pattern.trim().is_empty() {
                "*"
            } else {
                pattern
            };
            let found = table.matching(pattern);
            if found.is_empty() {
                self.cmdline
                    .say(format!("No variables match \"{}\".", pattern.trim()));
            }
            for var in found {
                let note = if var.read_only { " (read-only)" } else { "" };
                let line = format!(
                    "{} = {}{note}  {}",
                    var.name,
                    (var.get)(self),
                    var.description
                );
                self.cmdline.say(line);
            }
            self.cmdline.sysvar = None;
        } else if let Some(var) = table.find(text) {
            if var.read_only {
                let line = format!("{} = {} (read-only)", var.name, (var.get)(self));
                self.cmdline.say(line);
                self.cmdline.sysvar = None;
            } else {
                self.cmdline.sysvar = Some(SysVarPrompt::Value(var.name));
            }
        } else {
            self.cmdline
                .say(format!("Unknown variable name \"{text}\"."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_lines(state: &AppState, n: usize) -> Vec<String> {
        let log = &state.cmdline.log;
        log[log.len().saturating_sub(n)..].to_vec()
    }

    #[test]
    fn typing_a_name_asks_for_a_new_value_and_applies_it() {
        let mut state = AppState::new();
        assert_eq!(state.cmd_submit("gridmode"), None);
        assert_eq!(state.cmd_prompt().text, "Enter new value for GRIDMODE <1>");
        state.cmd_submit("0");
        assert!(!state.settings.show_grid);
        assert_eq!(last_lines(&state, 1), ["GRIDMODE = 0"]);
        assert!(state.cmdline.sysvar.is_none());
    }

    #[test]
    fn enter_keeps_the_value() {
        let mut state = AppState::new();
        state.cmd_submit("GRIDUNIT");
        assert_eq!(state.cmd_submit(""), None);
        assert!(state.cmdline.sysvar.is_none());
        assert_eq!(state.settings.grid_spacing, 10.0);
    }

    #[test]
    fn a_bad_value_asks_again() {
        let mut state = AppState::new();
        state.cmd_submit("GRIDMODE");
        state.cmd_submit("7");
        assert_eq!(last_lines(&state, 1), ["Requires a value from 0 to 1."]);
        assert!(state.cmdline.sysvar.is_some(), "still asking");
        assert!(state.settings.show_grid);
        state.cancel();
        assert!(state.cmdline.sysvar.is_none());
    }

    #[test]
    fn setvar_asks_for_the_name_and_lists_with_a_question_mark() {
        let mut state = AppState::new();
        state.begin_setvar();
        assert_eq!(
            state.cmd_prompt().plain(),
            "SETVAR Enter variable name or [?]:"
        );
        state.cmd_submit("nope");
        assert_eq!(last_lines(&state, 1), ["Unknown variable name \"nope\"."]);
        state.cmd_submit("?grid*");
        let log = last_lines(&state, 2).join("\n");
        assert!(log.contains("GRIDMODE = 1") && log.contains("GRIDUNIT = 10"));
        assert!(state.cmdline.sysvar.is_none());

        state.begin_setvar();
        state.cmd_submit("osmode");
        state.cmd_submit("1");
        assert!(state.settings.snap.endpoint);
        assert!(!state.settings.snap.midpoint);
    }

    #[test]
    fn read_only_variables_are_reported_not_asked_for() {
        let mut state = AppState::new();
        state.cmd_submit("cmdactive");
        assert_eq!(last_lines(&state, 1), ["CMDACTIVE = 0 (read-only)"]);
        assert!(state.cmdline.sysvar.is_none());
    }

    #[test]
    fn a_running_command_gets_the_text_first() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.cmd_submit("gridmode");
        assert!(state.cmdline.sysvar.is_none());
    }
}
