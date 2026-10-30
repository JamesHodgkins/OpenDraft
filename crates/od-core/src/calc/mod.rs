//! A calculator for named dimensions and trigonometry. A caller (a person or
//! an assistant) gives a list of steps, each a name and an expression that may
//! use the names before it, and gets exact numbers back: the arithmetic is
//! done here, in `f64`, and never by guesswork. Plain, strict and tested.
//!
//! Numbers, `+ - * / ^`, brackets, the constants `pi`, `tau` and `e`, and the
//! functions in [`FUNCTIONS`]. Angles are radians; `rad` and `deg` convert.
//! Anything that does not come out as a finite number (a division by zero,
//! the root of a negative) is an error that names the step, never a quiet
//! `inf` or `nan`.

mod parse;
#[cfg(test)]
mod tests;

use parse::{parse, Expr};

/// Most steps in one calculation, and longest expression.
pub const STEPS_MAX: usize = 100;
pub const EXPRESSION_MAX: usize = 500;
const NAME_MAX: usize = 32;

/// The functions, by name and how many arguments each takes (`None`: one or
/// more).
pub const FUNCTIONS: &[(&str, Option<usize>)] = &[
    ("sin", Some(1)),
    ("cos", Some(1)),
    ("tan", Some(1)),
    ("asin", Some(1)),
    ("acos", Some(1)),
    ("atan", Some(1)),
    ("atan2", Some(2)),
    ("sqrt", Some(1)),
    ("abs", Some(1)),
    ("floor", Some(1)),
    ("ceil", Some(1)),
    ("round", Some(1)),
    ("round_to", Some(2)),
    ("min", None),
    ("max", None),
    ("pow", Some(2)),
    ("hypot", Some(2)),
    ("exp", Some(1)),
    ("ln", Some(1)),
    ("log10", Some(1)),
    ("rad", Some(1)),
    ("deg", Some(1)),
];

const CONSTANTS: &[(&str, f64)] = &[
    ("pi", std::f64::consts::PI),
    ("tau", std::f64::consts::TAU),
    ("e", std::f64::consts::E),
];

/// Why a calculation could not be done, worded for whoever wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalcError(pub String);

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Work out each (`name`, `expression`) in order; a later expression may use
/// the names before it. The values come back in the same order.
pub fn evaluate(steps: &[(String, String)]) -> Result<Vec<(String, f64)>, CalcError> {
    if steps.is_empty() || steps.len() > STEPS_MAX {
        return Err(CalcError(format!(
            "a calculation has from 1 to {STEPS_MAX} steps"
        )));
    }
    let mut known: Vec<(String, f64)> = Vec::new();
    for (index, (name, text)) in steps.iter().enumerate() {
        let at = |why: CalcError| CalcError(format!("step {} ({name}): {why}", index + 1));
        check_name(name, &known).map_err(at)?;
        if text.chars().count() > EXPRESSION_MAX {
            return Err(at(CalcError(format!(
                "the expression is longer than {EXPRESSION_MAX} characters"
            ))));
        }
        let value = eval(&parse(text).map_err(at)?, &known).map_err(at)?;
        known.push((name.clone(), value));
    }
    Ok(known)
}

fn check_name(name: &str, known: &[(String, f64)]) -> Result<(), CalcError> {
    let plain = !name.is_empty()
        && name.len() <= NAME_MAX
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !plain {
        return Err(CalcError(format!(
            "a name is 1 to {NAME_MAX} letters, digits and underscores, not starting with a digit"
        )));
    }
    if CONSTANTS.iter().any(|(c, _)| *c == name) || FUNCTIONS.iter().any(|(f, _)| *f == name) {
        return Err(CalcError(format!(
            "{name} is already a constant or function"
        )));
    }
    if known.iter().any(|(n, _)| n == name) {
        return Err(CalcError(format!("{name} is defined twice")));
    }
    Ok(())
}

fn finite(value: f64, what: &str) -> Result<f64, CalcError> {
    if value.is_finite() {
        Ok(value)
    } else if value.is_nan() {
        Err(CalcError(format!("{what} is not a number")))
    } else {
        Err(CalcError(format!(
            "{what} is too large (or divides by zero)"
        )))
    }
}

fn eval(expr: &Expr, known: &[(String, f64)]) -> Result<f64, CalcError> {
    match expr {
        Expr::Number(n) => finite(*n, "a number"),
        Expr::Name(name) => known
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .or_else(|| CONSTANTS.iter().find(|(c, _)| c == name).map(|(_, v)| *v))
            .ok_or_else(|| CalcError(format!("{name} is not defined before this step"))),
        Expr::Negate(inner) => Ok(-eval(inner, known)?),
        Expr::Binary(op, left, right) => {
            let (a, b) = (eval(left, known)?, eval(right, known)?);
            match op {
                '+' => finite(a + b, "the sum"),
                '-' => finite(a - b, "the difference"),
                '*' => finite(a * b, "the product"),
                '/' => {
                    if b == 0.0 {
                        Err(CalcError("division by zero".to_owned()))
                    } else {
                        finite(a / b, "the quotient")
                    }
                }
                _ => finite(a.powf(b), "the power"),
            }
        }
        Expr::Call(name, args) => call(name, args, known),
    }
}

fn call(name: &str, args: &[Expr], known: &[(String, f64)]) -> Result<f64, CalcError> {
    let Some((_, arity)) = FUNCTIONS.iter().find(|(f, _)| *f == name) else {
        return Err(CalcError(format!("{name} is not a function")));
    };
    match arity {
        Some(n) if args.len() != *n => {
            return Err(CalcError(format!(
                "{name} takes {n} argument{}, not {}",
                if *n == 1 { "" } else { "s" },
                args.len()
            )));
        }
        None if args.is_empty() => {
            return Err(CalcError(format!("{name} needs at least one argument")));
        }
        _ => {}
    }
    let v: Vec<f64> = args
        .iter()
        .map(|a| eval(a, known))
        .collect::<Result<_, _>>()?;
    let result = match name {
        "sin" => v[0].sin(),
        "cos" => v[0].cos(),
        "tan" => v[0].tan(),
        "asin" => v[0].asin(),
        "acos" => v[0].acos(),
        "atan" => v[0].atan(),
        "atan2" => v[0].atan2(v[1]),
        "sqrt" => v[0].sqrt(),
        "abs" => v[0].abs(),
        "floor" => v[0].floor(),
        "ceil" => v[0].ceil(),
        "round" => v[0].round(),
        "round_to" => {
            let places = v[1].round();
            if !(0.0..=12.0).contains(&places) {
                return Err(CalcError("round_to places must be from 0 to 12".to_owned()));
            }
            let scale = 10f64.powi(places as i32);
            (v[0] * scale).round() / scale
        }
        "min" => v.iter().copied().fold(f64::INFINITY, f64::min),
        "max" => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "pow" => v[0].powf(v[1]),
        "hypot" => v[0].hypot(v[1]),
        "exp" => v[0].exp(),
        "ln" => v[0].ln(),
        "log10" => v[0].log10(),
        "rad" => v[0].to_radians(),
        _ => v[0].to_degrees(),
    };
    finite(result, &format!("{name}(...)"))
}
