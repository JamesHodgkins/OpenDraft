//! AutoCAD-style command line: type a command name or alias to launch it,
//! then answer its prompts with points, distances or option keywords.
//!
//! Pure logic over `AppState`; the panel in `app.rs` only draws it. Commands
//! themselves still run through the registry: `submit` hands back the id of
//! the command to run.

use std::borrow::Cow;

use od_core::Point;

use crate::state::AppState;

pub use crate::cmd_aliases::ALIASES;

const LOG_LIMIT: usize = 300;
const MAX_SUGGESTIONS: usize = 8;

/// Command-line state kept with the application.
#[derive(Default)]
pub struct CommandLine {
    pub input: String,
    /// Echoed commands, prompts with the answers given, and messages.
    pub log: Vec<String>,
    /// Highlighted suggestion row.
    pub selected: usize,
    /// How many suggestions were listed last frame (for arrow keys).
    pub listed: usize,
    /// Move keyboard focus to the input field on the next frame.
    pub focus: bool,
    /// Set by the panel's chevron: `Some(true)` collapses the panel to
    /// just the input row, `Some(false)` restores it. The app applies it.
    pub resize: Option<bool>,
    /// Panel share remembered while collapsed.
    pub saved_split: Option<f32>,
    /// The last drawing or editing command, repeated by Enter when idle.
    pub last_command: Option<&'static str>,
    /// Commands run lately, newest first, for the context menu's Recent Input.
    pub recent: Vec<&'static str>,
    /// Commands run (typed or from the ribbon), oldest first, for the
    /// Up and Down keys when no command is running.
    pub history: Vec<String>,
    /// The history line now in the input, while Up and Down are browsing.
    pub history_at: Option<usize>,
    /// A system variable prompt in progress (`SETVAR` or a variable name).
    pub(crate) sysvar: Option<crate::sysvar_line::SysVarPrompt>,
}

impl CommandLine {
    /// Remember a submitted line; a repeat of the last one is kept once.
    pub fn note_typed(&mut self, text: &str) {
        const KEPT: usize = 100;
        let text = text.trim();
        self.history_at = None;
        if text.is_empty() || self.history.last().is_some_and(|last| last == text) {
            return;
        }
        self.history.push(text.to_owned());
        if self.history.len() > KEPT {
            self.history.remove(0);
        }
    }

    /// Up (`older`) or Down moves through the typed lines, putting the line in
    /// `input`. Down past the newest line empties it. Returns false when there
    /// is nothing to show.
    pub fn browse_history(&mut self, older: bool, input: &mut String) -> bool {
        let last = self.history.len().checked_sub(1);
        let at = match (self.history_at, older) {
            (None, true) => last,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if Some(i) < last => Some(i + 1),
            (Some(_), false) => None,
            (None, false) => return false,
        };
        let Some(line) = at.and_then(|i| self.history.get(i)) else {
            self.history_at = None;
            input.clear();
            return older && last.is_some();
        };
        self.history_at = at;
        input.clone_from(line);
        true
    }

    pub fn say(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > LOG_LIMIT {
            self.log.drain(..self.log.len() - LOG_LIMIT);
        }
    }

    /// Echo a launched command, like AutoCAD's "Command: LINE".
    pub fn echo_command(&mut self, label: &str) {
        self.say(format!("Command: {}", label.to_uppercase()));
    }

    /// Remember a command run from the ribbon or the command line. Only
    /// commands that make something are kept, newest first, once each.
    pub fn note_recent(&mut self, id: &'static str) {
        const KEPT: usize = 6;
        // Enter repeats the last drawing or editing command.
        if ["draw.", "select.", "modify."]
            .iter()
            .any(|prefix| id.starts_with(prefix))
        {
            self.last_command = Some(id);
        }
        if ["edit.", "view.", "file.", "tool.", "layer."]
            .iter()
            .any(|prefix| id.starts_with(prefix))
        {
            return;
        }
        self.recent.retain(|r| *r != id);
        self.recent.insert(0, id);
        self.recent.truncate(KEPT);
    }
}

/// The name a command is echoed by: its AutoCAD name from [`ALIASES`]
/// (LAYOFF, not Hide), or its label when it has none.
pub fn echo_name<'a>(id: &str, label: &'a str) -> &'a str {
    ALIASES
        .iter()
        .find(|(_, _, command)| *command == id)
        .map_or(label, |(_, name, _)| name)
}

/// A keyword the user can type, or click, at the current prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CmdOption {
    pub label: Cow<'static, str>,
    /// Letters that pick it when typed (shown highlighted).
    pub key: Cow<'static, str>,
}

/// The line shown next to the input: `COMMAND text [Option/Option]:`.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub command: Cow<'static, str>,
    pub text: Cow<'static, str>,
    pub options: Vec<CmdOption>,
}

impl Prompt {
    /// Plain-text form, as written to the log.
    pub fn plain(&self) -> String {
        if self.command.is_empty() {
            return self.text.to_string();
        }
        let options = if self.options.is_empty() {
            String::new()
        } else {
            let labels: Vec<_> = self.options.iter().map(|o| &*o.label).collect();
            format!(" [{}]", labels.join("/"))
        };
        format!("{} {}{}:", self.command, self.text, options)
    }
}

/// Parse typed coordinates: `x,y`, `@dx,dy`, `dist<angle`, `@dist<angle`
/// (lengths and angles in the drawing's units). `base` is the origin for `@`.
pub fn parse_point(text: &str, base: Point, units: &od_core::Units) -> Option<Point> {
    crate::coordinates::parse_point_with_mode(
        text,
        base,
        crate::coordinates::CoordinateMode::CommandLine,
        units,
    )
}

pub fn parse_number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A command offered while typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub alias: &'static str,
    pub name: &'static str,
    pub command: &'static str,
}

impl Suggestion {
    /// "L (LINE)", or just the name when there is no separate alias.
    pub fn label(&self) -> String {
        if self.alias == self.name {
            self.name.to_owned()
        } else {
            format!("{} ({})", self.alias, self.name)
        }
    }
}

/// Commands matching what has been typed, best first. `enabled` hides
/// commands that cannot run right now.
pub fn suggestions(text: &str, enabled: impl Fn(&str) -> bool) -> Vec<Suggestion> {
    let t = text.trim().to_uppercase();
    if t.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(u8, Suggestion)> = ALIASES
        .iter()
        .filter(|(_, _, command)| enabled(command))
        .filter_map(|&(alias, name, command)| {
            let rank = if alias == t || name.eq_ignore_ascii_case(&t) {
                0
            } else if alias.starts_with(&t) {
                1
            } else if name.starts_with(&t) {
                2
            } else if t.len() >= 3 && name.contains(&t) {
                3
            } else {
                return None;
            };
            Some((
                rank,
                Suggestion {
                    alias,
                    name,
                    command,
                },
            ))
        })
        .collect();
    found.sort_by_key(|(rank, s)| (*rank, s.alias.len()));
    let mut out: Vec<Suggestion> = Vec::new();
    for (_, s) in found {
        if !out.iter().any(|o| o.command == s.command) {
            out.push(s);
        }
    }
    out.truncate(MAX_SUGGESTIONS);
    out
}

/// The command a fully typed alias or name stands for.
fn command_for(text: &str) -> Option<&'static str> {
    let t = text.trim().to_uppercase();
    ALIASES
        .iter()
        .find(|(alias, name, _)| *alias == t || *name == t)
        .map(|(_, _, command)| *command)
}

impl AppState {
    /// What the command line is asking for right now.
    pub fn cmd_prompt(&mut self) -> Prompt {
        if let Some(prompt) = self.runner_prompt().or_else(|| self.sysvar_prompt()) {
            return prompt;
        }
        Prompt {
            command: Cow::Borrowed(""),
            text: Cow::Borrowed("Type a command"),
            options: Vec::new(),
        }
    }

    /// The option `text` picks at the current prompt, by key or name.
    pub fn cmd_option_for(&mut self, text: &str) -> Option<CmdOption> {
        let t = text.trim().to_uppercase();
        if t.is_empty() {
            return None;
        }
        self.cmd_prompt()
            .options
            .into_iter()
            .find(|o| o.key.to_uppercase() == t || o.label.to_uppercase().starts_with(&t))
    }

    /// Commands to suggest for `text`: only while it looks like a command
    /// name (not a number, coordinate or one of the prompt's options).
    pub fn cmd_suggestions(
        &mut self,
        text: &str,
        enabled: impl Fn(&str) -> bool,
    ) -> Vec<Suggestion> {
        let starts_alpha = text.trim().chars().next().is_some_and(char::is_alphabetic);
        if !starts_alpha || self.cmd_option_for(text).is_some() {
            return Vec::new();
        }
        suggestions(text, enabled)
    }

    /// Choose an option (typed or clicked).
    pub fn cmd_choose(&mut self, option: CmdOption) {
        let line = format!("{} {}", self.cmd_prompt().plain(), option.label);
        self.cmdline.say(line);
        if self.command_running() {
            self.answer_keyword(&option.label);
        }
    }

    /// Enter on an empty line: answer the running command's prompt, or
    /// repeat the last command when idle.
    pub fn cmd_enter(&mut self) -> Option<&'static str> {
        if self.command_running() {
            self.answer_enter();
            return None;
        }
        self.cmdline.last_command
    }

    /// Process one line of typed input. Returns a command id for the app
    /// to run when the text names a command.
    pub fn cmd_submit(&mut self, text: &str) -> Option<&'static str> {
        let t = text.trim();
        if self.sysvar_submit(t) {
            return None;
        }
        if t.is_empty() {
            return self.cmd_enter();
        }
        if self.answer_typed(t) {
            return None;
        }
        // Coordinates typed with no command to take them.
        if parse_point(t, Point::new(0.0, 0.0), &self.units()).is_some() {
            let line = format!("{} {t}", self.cmd_prompt().plain());
            self.cmdline.say(line);
            self.cmdline.say("Start a drawing command first.");
            return None;
        }

        if let Some(option) = self.cmd_option_for(t) {
            self.cmd_choose(option);
            return None;
        }

        if let Some(command) = command_for(t) {
            return Some(command);
        }
        if self.sysvar_typed(t) {
            return None;
        }
        self.cmdline.say(format!("Unknown command \"{t}\"."));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::{Entity, Units};

    #[test]
    fn commands_echo_by_their_autocad_names() {
        assert_eq!(echo_name("layer.hide", "Hide"), "LAYOFF");
        assert_eq!(echo_name("layer.unlock", "Unlock"), "LAYULK");
        assert_eq!(echo_name("annotate.mtext", "Multiline Text"), "MTEXT");
        assert_eq!(
            echo_name("view.reset_layout", "Reset Layout"),
            "Reset Layout"
        );
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    #[test]
    fn a_full_command_name_ranks_first_among_longer_matches() {
        // History brings back names such as LINE, which LINETYPE also starts with.
        for (_, name, command) in ALIASES {
            let found = suggestions(name, |_| true);
            assert_eq!(found.first().map(|s| s.command), Some(*command), "{name}");
        }
    }

    #[test]
    fn up_and_down_walk_the_typed_lines() {
        let mut line = CommandLine::default();
        let mut input = String::new();
        assert!(!line.browse_history(true, &mut input));
        for text in ["L", "C", "C", "M"] {
            line.note_typed(text);
        }
        assert_eq!(line.history, ["L", "C", "M"]);
        line.browse_history(true, &mut input);
        assert_eq!(input, "M");
        line.browse_history(true, &mut input);
        line.browse_history(true, &mut input);
        line.browse_history(true, &mut input);
        assert_eq!(input, "L", "stops at the oldest");
        line.browse_history(false, &mut input);
        assert_eq!(input, "C");
        line.browse_history(false, &mut input);
        line.browse_history(false, &mut input);
        assert_eq!(input, "", "Down past the newest empties the line");
        assert_eq!(line.history_at, None);
    }

    #[test]
    fn recent_commands_are_newest_first_once_each_and_skip_settings() {
        let mut line = CommandLine::default();
        for id in ["draw.line", "modify.move", "view.grid", "draw.line"] {
            line.note_recent(id);
        }
        assert_eq!(line.recent, ["draw.line", "modify.move"]);
    }

    #[test]
    fn parses_absolute_relative_and_polar_points() {
        let base = p(10.0, 10.0);
        assert_eq!(
            parse_point("3,4", base, &Units::default()),
            Some(p(3.0, 4.0))
        );
        assert_eq!(
            parse_point("@3,-4", base, &Units::default()),
            Some(p(13.0, 6.0))
        );
        assert_eq!(
            parse_point(" 1 , 2 ", base, &Units::default()),
            Some(p(1.0, 2.0))
        );
        let polar = parse_point("@10<90", base, &Units::default()).unwrap();
        assert!((polar.x - 10.0).abs() < 1e-9 && (polar.y - 20.0).abs() < 1e-9);
        let abs_polar = parse_point("5<0", base, &Units::default()).unwrap();
        assert!((abs_polar.x - 5.0).abs() < 1e-9 && abs_polar.y.abs() < 1e-9);
    }

    #[test]
    fn rejects_things_that_are_not_points() {
        let base = p(0.0, 0.0);
        for text in ["", "5", "abc", "1,", ",2", "1,x", "@", "nan,1", "inf<2"] {
            assert_eq!(parse_point(text, base, &Units::default()), None, "{text}");
        }
        assert_eq!(parse_number("12.5"), Some(12.5));
        assert_eq!(parse_number("nan"), None);
        assert_eq!(parse_number("inf"), None);
    }

    #[test]
    fn suggestions_rank_exact_alias_first_and_skip_disabled() {
        let all = |_: &str| true;
        let s = suggestions("l", all);
        assert_eq!(s[0].command, "draw.line");
        assert_eq!(s[0].label(), "L (LINE)");
        // "REC" and "RECTANG" both map to the same command: listed once.
        let rec = suggestions("rec", all);
        assert_eq!(
            rec.iter().filter(|s| s.command == "draw.rectangle").count(),
            1
        );
        // Name prefixes and substrings match too.
        assert!(suggestions("circ", all)
            .iter()
            .any(|s| s.command == "draw.circle"));
        assert!(suggestions("extents", all)
            .iter()
            .any(|s| s.command == "view.zoom_extents"));
        assert!(suggestions("", all).is_empty());
        assert!(suggestions("l", |id| id != "draw.line")
            .iter()
            .all(|s| s.command != "draw.line"));
        assert!(suggestions("zzz", all).is_empty());
    }

    #[test]
    fn every_alias_points_at_a_real_command() {
        let registry = crate::commands::build_registry();
        for (alias, _, command) in ALIASES {
            assert!(registry.get(command).is_some(), "{alias} -> {command}");
        }
    }

    #[test]
    fn typing_a_command_name_returns_its_id() {
        let mut state = AppState::new();
        assert_eq!(state.cmd_submit("l"), Some("draw.line"));
        assert_eq!(state.cmd_submit("CIRCLE"), Some("draw.circle"));
        assert_eq!(state.cmd_submit("nonsense"), None);
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("Unknown command"));
    }

    #[test]
    fn dimension_commands_are_found_by_their_names() {
        assert_eq!(command_for("DIMSTYLE"), Some("dim.style"));
        assert_eq!(command_for("-DIMSTYLE"), Some("dim.update"));
        assert_eq!(command_for("DIMEDIT"), Some("dim.edit"));
        assert_eq!(command_for("DOV"), Some("dim.override"));
    }

    #[test]
    fn f_and_cp_follow_autocad() {
        assert_eq!(command_for("F"), Some("modify.fillet"));
        assert_eq!(command_for("CP"), Some("modify.copy"));
        assert_eq!(command_for("FENCE"), Some("select.fence"));
        assert_eq!(command_for("CPOLYGON"), Some("select.crossing_polygon"));
    }

    #[test]
    fn enter_ends_a_drawing_command_then_repeats_it_when_idle() {
        let mut state = AppState::new();
        state.cmdline.last_command = Some("draw.line");
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        // Enter at the first point ends the command; at rest it repeats it.
        assert_eq!(state.cmd_enter(), None);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(state.cmd_enter(), Some("draw.line"));
    }

    #[test]
    fn points_are_ignored_outside_a_command() {
        let mut state = AppState::new();
        state.cmd_submit("1,1");
        assert!(entities(&state).is_empty());
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("Start a drawing"));
    }

    #[test]
    fn suggestions_stand_down_for_numbers_and_prompt_options() {
        let mut state = AppState::new();
        let all = |_: &str| true;
        assert!(state.cmd_suggestions("12", all).is_empty());
        assert!(state.cmd_suggestions("1,2", all).is_empty());
        assert!(!state.cmd_suggestions("l", all).is_empty());
        state.start_command("Circle", crate::circle::draw_circle);
        state.settle_command();
        state.cmd_submit("0,0");
        state.settle_command();
        // "D" is the Diameter option here, not the start of a command name.
        assert!(state.cmd_suggestions("d", all).is_empty());
        assert!(!state.cmd_suggestions("l", all).is_empty());
    }

    #[test]
    fn prompts_read_like_autocad() {
        let mut state = AppState::new();
        assert_eq!(state.cmd_prompt().plain(), "Type a command");
        state.start_command("Fence", crate::selection::fence);
        state.settle_command();
        assert_eq!(
            state.cmd_prompt().plain(),
            "FENCE Specify first fence point or [Remove]:"
        );
    }

    #[test]
    fn log_is_capped() {
        let mut cl = CommandLine::default();
        for i in 0..LOG_LIMIT + 50 {
            cl.say(format!("line {i}"));
        }
        assert_eq!(cl.log.len(), LOG_LIMIT);
        assert_eq!(cl.log.last().unwrap(), &format!("line {}", LOG_LIMIT + 49));
    }

    #[test]
    fn modify_aliases_exist() {
        for alias in ["m", "co", "ro", "mi"] {
            assert!(command_for(alias).is_some(), "{alias}");
        }
    }

    #[test]
    fn power_select_answers_to_its_own_and_autocads_names() {
        for alias in ["powerselect", "pse", "qselect", "qse"] {
            assert_eq!(command_for(alias), Some("select.power"), "{alias}");
        }
    }
}
