use std::collections::HashMap;
use std::fmt;

/// Whether an out-of-process caller (the MCP server) may run a command, and
/// how. The default is `Denied`: a command is reachable only when someone
/// has decided it is safe and said so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutomationPolicy {
    #[default]
    Denied,
    /// Does its work at once with no questions, so it can be run on request.
    ImmediateSafe,
    /// Asks the user for points and choices; it can only be started with
    /// the prompts staying in the app.
    InteractiveInApp,
}

/// What a command does, for deciding who may run it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommandEffect {
    ReadOnly,
    /// View, modes, selection and panels: nothing in the drawing changes.
    #[default]
    UiState,
    DrawingMutation,
    FileRead,
    FileWrite,
    /// Opening dialogs, closing drawings, quitting.
    ApplicationLifecycle,
}

/// A named, user-invocable action. `C` is the context commands run against
/// (the application state); the registry itself knows nothing about it.
pub struct Command<C> {
    /// Stable dotted id, e.g. `"edit.undo"`. Ribbon items refer to this.
    pub id: &'static str,
    pub label: &'static str,
    /// Default shortcut in text form, e.g. `"Ctrl+Z"`. `Ctrl` means Cmd on
    /// macOS. Parsing is left to the UI layer.
    pub shortcut: Option<&'static str>,
    pub enabled: fn(&C) -> bool,
    /// Whether the command's mode is on (a command running, a toggle); the UI
    /// draws such commands lit.
    pub checked: fn(&C) -> bool,
    pub run: fn(&mut C),
    /// May run while another command is waiting for the user (zoom, panels,
    /// cancel). Every other command is unavailable until that one ends, so
    /// the drawing cannot change under it. See `Registry::busy_when`.
    pub transparent: bool,
    /// Works with no drawing in front (file, window and settings commands).
    /// See `Registry::needs_drawing_when`.
    pub global: bool,
    /// Who besides the user may run it. Denied unless classified.
    pub automation: AutomationPolicy,
    pub effect: CommandEffect,
}

impl<C> Command<C> {
    /// An always-enabled, unchecked command with no shortcut.
    pub fn new(id: &'static str, label: &'static str, run: fn(&mut C)) -> Self {
        Self {
            id,
            label,
            shortcut: None,
            enabled: always,
            checked: never,
            run,
            transparent: false,
            global: false,
            automation: AutomationPolicy::Denied,
            effect: CommandEffect::UiState,
        }
    }

    /// Say whether automation may run it, and what it does.
    pub fn automation(mut self, policy: AutomationPolicy, effect: CommandEffect) -> Self {
        self.automation = policy;
        self.effect = effect;
        self
    }

    pub fn key(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Only runnable while `enabled` holds.
    pub fn when(mut self, enabled: fn(&C) -> bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn checked(mut self, checked: fn(&C) -> bool) -> Self {
        self.checked = checked;
        self
    }

    /// Allowed while another command is running.
    pub fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }

    /// Allowed even when no drawing is in front.
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }
}

/// `enabled` predicate for commands that are always available.
pub fn always<C>(_: &C) -> bool {
    true
}

/// `checked` predicate for commands that are not modes.
pub fn never<C>(_: &C) -> bool {
    false
}

#[derive(Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateId(&'static str),
    DuplicateShortcut {
        shortcut: &'static str,
        existing: &'static str,
        new: &'static str,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateId(id) => write!(f, "command id `{id}` registered twice"),
            RegistryError::DuplicateShortcut {
                shortcut,
                existing,
                new,
            } => write!(
                f,
                "shortcut `{shortcut}` for `{new}` is already used by `{existing}`"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug, PartialEq, Eq)]
pub enum ExecuteError {
    Unknown(String),
    Disabled(&'static str),
}

pub struct Registry<C> {
    commands: Vec<Command<C>>,
    by_id: HashMap<&'static str, usize>,
    busy: Option<fn(&C) -> bool>,
    no_drawing: Option<fn(&C) -> bool>,
}

impl<C> Default for Registry<C> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            by_id: HashMap::new(),
            busy: None,
            no_drawing: None,
        }
    }
}

impl<C> Registry<C> {
    /// Add a command. Ids and shortcuts must be unique.
    pub fn register(&mut self, command: Command<C>) -> Result<(), RegistryError> {
        if self.by_id.contains_key(command.id) {
            return Err(RegistryError::DuplicateId(command.id));
        }
        if let Some(shortcut) = command.shortcut {
            let existing = self
                .commands
                .iter()
                .find(|c| c.shortcut.is_some_and(|s| s.eq_ignore_ascii_case(shortcut)));
            if let Some(existing) = existing {
                return Err(RegistryError::DuplicateShortcut {
                    shortcut,
                    existing: existing.id,
                    new: command.id,
                });
            }
        }
        self.by_id.insert(command.id, self.commands.len());
        self.commands.push(command);
        Ok(())
    }

    /// While `busy` holds (a command is waiting for the user), only
    /// `transparent` commands are enabled.
    pub fn busy_when(&mut self, busy: fn(&C) -> bool) {
        self.busy = Some(busy);
    }

    /// While `no_drawing` holds (the Home page is showing), only `global`
    /// commands are enabled.
    pub fn needs_drawing_when(&mut self, no_drawing: fn(&C) -> bool) {
        self.no_drawing = Some(no_drawing);
    }

    fn available(&self, command: &Command<C>, ctx: &C) -> bool {
        let blocked = !command.transparent && self.busy.is_some_and(|busy| busy(ctx));
        let homeless = !command.global && self.no_drawing.is_some_and(|f| f(ctx));
        !blocked && !homeless && (command.enabled)(ctx)
    }

    pub fn get(&self, id: &str) -> Option<&Command<C>> {
        self.by_id.get(id).map(|&i| &self.commands[i])
    }

    /// All commands in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Command<C>> {
        self.commands.iter()
    }

    /// Whether `id` exists and is currently enabled. Unknown ids are not.
    pub fn is_enabled(&self, id: &str, ctx: &C) -> bool {
        self.get(id).is_some_and(|c| self.available(c, ctx))
    }

    /// Whether `id` exists and its mode is currently on.
    pub fn is_checked(&self, id: &str, ctx: &C) -> bool {
        self.get(id).is_some_and(|c| (c.checked)(ctx))
    }

    pub fn execute(&self, id: &str, ctx: &mut C) -> Result<(), ExecuteError> {
        let command = self
            .get(id)
            .ok_or_else(|| ExecuteError::Unknown(id.to_owned()))?;
        if !self.available(command, ctx) {
            return Err(ExecuteError::Disabled(command.id));
        }
        (command.run)(ctx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Ctx {
        counter: i32,
        allow: bool,
    }

    fn inc(ctx: &mut Ctx) {
        ctx.counter += 1;
    }

    fn cmd(id: &'static str, shortcut: Option<&'static str>) -> Command<Ctx> {
        let command = Command::new(id, id, inc);
        match shortcut {
            Some(s) => command.key(s),
            None => command,
        }
    }

    #[test]
    fn executes_registered_command() {
        let mut registry = Registry::default();
        registry.register(cmd("a", None)).unwrap();
        let mut ctx = Ctx::default();
        registry.execute("a", &mut ctx).unwrap();
        assert_eq!(ctx.counter, 1);
    }

    #[test]
    fn checked_follows_the_predicate() {
        let mut registry = Registry::default();
        registry
            .register(cmd("a", None).checked(|c: &Ctx| c.allow))
            .unwrap();
        let mut ctx = Ctx::default();
        assert!(!registry.is_checked("a", &ctx));
        ctx.allow = true;
        assert!(registry.is_checked("a", &ctx));
        assert!(!registry.is_checked("nope", &ctx));
    }

    #[test]
    fn unknown_command_is_an_error() {
        let registry = Registry::<Ctx>::default();
        let mut ctx = Ctx::default();
        assert_eq!(
            registry.execute("nope", &mut ctx),
            Err(ExecuteError::Unknown("nope".to_owned()))
        );
        assert!(!registry.is_enabled("nope", &ctx));
    }

    #[test]
    fn disabled_command_does_not_run() {
        let mut registry = Registry::default();
        registry
            .register(cmd("a", None).when(|c: &Ctx| c.allow))
            .unwrap();
        let mut ctx = Ctx::default();
        assert_eq!(
            registry.execute("a", &mut ctx),
            Err(ExecuteError::Disabled("a"))
        );
        assert_eq!(ctx.counter, 0);

        ctx.allow = true;
        assert!(registry.is_enabled("a", &ctx));
        registry.execute("a", &mut ctx).unwrap();
        assert_eq!(ctx.counter, 1);
    }

    #[test]
    fn only_transparent_commands_run_while_busy() {
        let mut registry = Registry::default();
        registry.register(cmd("a", None)).unwrap();
        registry
            .register(Command::new("zoom", "zoom", inc).transparent())
            .unwrap();
        registry.busy_when(|c: &Ctx| c.allow);
        let mut ctx = Ctx::default();
        assert!(registry.is_enabled("a", &ctx));
        ctx.allow = true;
        assert!(!registry.is_enabled("a", &ctx));
        assert_eq!(
            registry.execute("a", &mut ctx),
            Err(ExecuteError::Disabled("a"))
        );
        assert!(registry.is_enabled("zoom", &ctx));
        registry.execute("zoom", &mut ctx).unwrap();
        assert_eq!(ctx.counter, 1);
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let mut registry = Registry::default();
        registry.register(cmd("a", None)).unwrap();
        assert_eq!(
            registry.register(cmd("a", None)),
            Err(RegistryError::DuplicateId("a"))
        );
    }

    #[test]
    fn duplicate_shortcut_is_rejected_case_insensitively() {
        let mut registry = Registry::default();
        registry.register(cmd("a", Some("Ctrl+Z"))).unwrap();
        assert_eq!(
            registry.register(cmd("b", Some("ctrl+z"))),
            Err(RegistryError::DuplicateShortcut {
                shortcut: "ctrl+z",
                existing: "a",
                new: "b"
            })
        );
    }

    #[test]
    fn only_global_commands_run_with_no_drawing() {
        let mut registry = Registry::default();
        registry.needs_drawing_when(|ctx: &Ctx| ctx.allow);
        registry.register(cmd("draw", None)).unwrap();
        registry.register(cmd("open", None).global()).unwrap();
        let mut ctx = Ctx::default();
        assert!(registry.is_enabled("draw", &ctx));
        ctx.allow = true;
        assert!(!registry.is_enabled("draw", &ctx));
        assert!(registry.is_enabled("open", &ctx));
        assert_eq!(
            registry.execute("draw", &mut ctx),
            Err(ExecuteError::Disabled("draw"))
        );
    }
}
