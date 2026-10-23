//! System variables: named, typed settings read and set by name, the way
//! AutoCAD's `SETVAR` does. The table knows nothing about the application;
//! like the command [`Registry`](crate::Registry) it is generic over the
//! context `C` that its getters and setters work on.

use std::fmt;

/// A variable's value. Switches are integers (0 or 1), as in AutoCAD.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Real(f64),
    Text(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Real(n) => write!(f, "{n}"),
            Value::Text(s) => write!(f, "\"{s}\""),
        }
    }
}

/// What a variable holds, and the range it accepts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Int { min: i64, max: i64 },
    Real { min: f64, max: f64 },
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysVarError {
    Unknown(String),
    ReadOnly(&'static str),
    NotAnInteger,
    NotANumber,
    OutOfRange(String),
    /// The setter refused the value; the text says why.
    Rejected(String),
}

impl fmt::Display for SysVarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SysVarError::Unknown(name) => write!(f, "Unknown variable name \"{name}\"."),
            SysVarError::ReadOnly(name) => write!(f, "{name} is read-only."),
            SysVarError::NotAnInteger => write!(f, "Requires an integer value."),
            SysVarError::NotANumber => write!(f, "Requires a numeric value."),
            SysVarError::OutOfRange(range) => write!(f, "Requires a value {range}."),
            SysVarError::Rejected(why) => f.write_str(why),
        }
    }
}

/// One variable: its name, what it holds, and how to read and set it.
pub struct SysVar<C> {
    /// Upper case; lookup ignores case.
    pub name: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    /// Read-only variables report state the user cannot set.
    pub read_only: bool,
    pub get: fn(&C) -> Value,
    /// Applies a value already checked against `kind`. May still refuse it.
    pub set: fn(&mut C, Value) -> Result<(), String>,
}

fn unsettable<C>(_: &mut C, _: Value) -> Result<(), String> {
    Err(String::new())
}

impl<C> SysVar<C> {
    pub fn int(
        name: &'static str,
        description: &'static str,
        range: (i64, i64),
        get: fn(&C) -> Value,
        set: fn(&mut C, Value) -> Result<(), String>,
    ) -> Self {
        Self {
            name,
            description,
            kind: Kind::Int {
                min: range.0,
                max: range.1,
            },
            read_only: false,
            get,
            set,
        }
    }

    pub fn real(
        name: &'static str,
        description: &'static str,
        range: (f64, f64),
        get: fn(&C) -> Value,
        set: fn(&mut C, Value) -> Result<(), String>,
    ) -> Self {
        Self {
            name,
            description,
            kind: Kind::Real {
                min: range.0,
                max: range.1,
            },
            read_only: false,
            get,
            set,
        }
    }

    /// A variable that reports state and cannot be set.
    pub fn read_only(
        name: &'static str,
        description: &'static str,
        kind: Kind,
        get: fn(&C) -> Value,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            read_only: true,
            get,
            set: unsettable,
        }
    }

    /// Turn typed text into a value of this variable's kind, in range.
    pub fn parse(&self, text: &str) -> Result<Value, SysVarError> {
        let text = text.trim();
        match self.kind {
            Kind::Int { min, max } => {
                let n: i64 = text.parse().map_err(|_| SysVarError::NotAnInteger)?;
                if (min..=max).contains(&n) {
                    Ok(Value::Int(n))
                } else {
                    Err(SysVarError::OutOfRange(format!("from {min} to {max}")))
                }
            }
            Kind::Real { min, max } => {
                let n: f64 = text
                    .parse()
                    .ok()
                    .filter(|n: &f64| n.is_finite())
                    .ok_or(SysVarError::NotANumber)?;
                if (min..=max).contains(&n) {
                    Ok(Value::Real(n))
                } else {
                    Err(SysVarError::OutOfRange(format!("from {min} to {max}")))
                }
            }
            Kind::Text => Ok(Value::Text(text.to_owned())),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TableError {
    DuplicateName(&'static str),
    /// Names are typed at the command line, so they are letters, digits and
    /// underscores in upper case.
    BadName(&'static str),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::DuplicateName(name) => write!(f, "system variable {name} defined twice"),
            TableError::BadName(name) => write!(f, "bad system variable name {name:?}"),
        }
    }
}

/// All the system variables, in name order.
pub struct SysVarTable<C> {
    vars: Vec<SysVar<C>>,
}

impl<C> SysVarTable<C> {
    pub fn new(mut vars: Vec<SysVar<C>>) -> Result<Self, TableError> {
        vars.sort_by_key(|v| v.name);
        for v in &vars {
            let ok = !v.name.is_empty()
                && v.name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                return Err(TableError::BadName(v.name));
            }
        }
        if let Some(pair) = vars.windows(2).find(|w| w[0].name == w[1].name) {
            return Err(TableError::DuplicateName(pair[0].name));
        }
        Ok(Self { vars })
    }

    pub fn find(&self, name: &str) -> Option<&SysVar<C>> {
        let name = name.trim();
        self.vars.iter().find(|v| v.name.eq_ignore_ascii_case(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &SysVar<C>> {
        self.vars.iter()
    }

    /// Variables whose names match `pattern`, where `*` stands for any run
    /// of characters (`GRID*`, `*MODE`, `*`). Case does not matter.
    pub fn matching(&self, pattern: &str) -> Vec<&SysVar<C>> {
        let pattern = pattern.trim().to_ascii_uppercase();
        self.vars
            .iter()
            .filter(|v| wildcard_match(&pattern, v.name))
            .collect()
    }

    pub fn get(&self, name: &str, ctx: &C) -> Result<Value, SysVarError> {
        let var = self.find_or_err(name)?;
        Ok((var.get)(ctx))
    }

    /// Parse `text` for the variable and set it. Returns the value now held.
    pub fn set(&self, name: &str, text: &str, ctx: &mut C) -> Result<Value, SysVarError> {
        let var = self.find_or_err(name)?;
        if var.read_only {
            return Err(SysVarError::ReadOnly(var.name));
        }
        let value = var.parse(text)?;
        (var.set)(ctx, value).map_err(SysVarError::Rejected)?;
        Ok((var.get)(ctx))
    }

    fn find_or_err(&self, name: &str) -> Result<&SysVar<C>, SysVarError> {
        self.find(name)
            .ok_or_else(|| SysVarError::Unknown(name.trim().to_owned()))
    }
}

/// `pattern` (upper case, `*` = any run) against `text`.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || text.len() < first.len() + last.len() || !text.ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(at) => rest = &rest[at + middle.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Ctx {
        on: i64,
        size: f64,
    }

    fn table() -> SysVarTable<Ctx> {
        SysVarTable::new(vec![
            SysVar::int(
                "GRIDMODE",
                "Grid on or off",
                (0, 1),
                |c: &Ctx| Value::Int(c.on),
                |c, v| {
                    if let Value::Int(n) = v {
                        c.on = n;
                    }
                    Ok(())
                },
            ),
            SysVar::real(
                "GRIDUNIT",
                "Grid spacing",
                (1.0, 100.0),
                |c: &Ctx| Value::Real(c.size),
                |c, v| match v {
                    Value::Real(13.0) => Err("Unlucky.".to_owned()),
                    Value::Real(n) => {
                        c.size = n;
                        Ok(())
                    }
                    _ => Ok(()),
                },
            ),
            SysVar::read_only("DWGNAME", "Name", Kind::Text, |_: &Ctx| {
                Value::Text("a.odx".to_owned())
            }),
        ])
        .unwrap()
    }

    #[test]
    fn lookup_ignores_case_and_the_table_is_sorted() {
        let t = table();
        assert!(t.find("gridmode").is_some());
        assert!(t.find(" GridUnit ").is_some());
        assert!(t.find("nope").is_none());
        let names: Vec<_> = t.iter().map(|v| v.name).collect();
        assert_eq!(names, ["DWGNAME", "GRIDMODE", "GRIDUNIT"]);
    }

    #[test]
    fn set_parses_checks_the_range_and_returns_the_new_value() {
        let (t, mut c) = (table(), Ctx::default());
        assert_eq!(t.set("gridmode", "1", &mut c), Ok(Value::Int(1)));
        assert_eq!(c.on, 1);
        assert_eq!(t.set("GRIDUNIT", " 2.5 ", &mut c), Ok(Value::Real(2.5)));
        assert_eq!(
            t.set("GRIDMODE", "2", &mut c),
            Err(SysVarError::OutOfRange("from 0 to 1".into()))
        );
        assert_eq!(
            t.set("GRIDMODE", "1.5", &mut c),
            Err(SysVarError::NotAnInteger)
        );
        assert_eq!(
            t.set("GRIDUNIT", "nan", &mut c),
            Err(SysVarError::NotANumber)
        );
        assert_eq!(c.on, 1, "a refused value changes nothing");
    }

    #[test]
    fn the_setter_can_refuse_a_value() {
        let (t, mut c) = (table(), Ctx::default());
        assert_eq!(
            t.set("GRIDUNIT", "13", &mut c),
            Err(SysVarError::Rejected("Unlucky.".into()))
        );
        assert_eq!(c.size, 0.0);
    }

    #[test]
    fn read_only_variables_report_but_do_not_set() {
        let (t, mut c) = (table(), Ctx::default());
        assert_eq!(t.get("dwgname", &c), Ok(Value::Text("a.odx".into())));
        assert_eq!(
            t.set("DWGNAME", "x", &mut c),
            Err(SysVarError::ReadOnly("DWGNAME"))
        );
        assert_eq!(
            t.set("FOO", "1", &mut c),
            Err(SysVarError::Unknown("FOO".into()))
        );
    }

    #[test]
    fn wildcards_match_prefix_suffix_and_middle() {
        let t = table();
        let names = |p: &str| t.matching(p).iter().map(|v| v.name).collect::<Vec<_>>();
        assert_eq!(names("*"), ["DWGNAME", "GRIDMODE", "GRIDUNIT"]);
        assert_eq!(names("grid*"), ["GRIDMODE", "GRIDUNIT"]);
        assert_eq!(names("*MODE"), ["GRIDMODE"]);
        assert_eq!(names("G*D*E"), ["GRIDMODE"]);
        assert_eq!(names("GRIDMODE"), ["GRIDMODE"]);
        assert!(names("GRID").is_empty());
    }

    #[test]
    fn a_bad_table_is_refused() {
        let var = |name| SysVar::read_only(name, "", Kind::Text, |_: &Ctx| Value::Int(0));
        assert!(matches!(
            SysVarTable::new(vec![var("A"), var("A")]),
            Err(TableError::DuplicateName("A"))
        ));
        assert!(matches!(
            SysVarTable::new(vec![var("lower")]),
            Err(TableError::BadName("lower"))
        ));
    }
}
