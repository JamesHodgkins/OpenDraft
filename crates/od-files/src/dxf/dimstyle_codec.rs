//! The dimension style variables as DXF group codes, in both directions: a
//! DIMSTYLE table record carries them, and a dimension carries the ones it
//! overrides as `ACAD`/`DSTYLE` extended data. Only the variables
//! `DimStyle` keeps are here; the rest of a record is AutoCAD's defaults on
//! the way out and ignored on the way in.

use od_core::{AngleFormat, Arrow, DimStyle, LengthFormat};

/// A variable's value as it is written.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Real(f64),
    Int(i64),
    Text(String),
}

struct Entry {
    /// The DIMSTYLE group code (also the code in extended data).
    code: i32,
    get: fn(&DimStyle) -> Value,
    set: fn(&mut DimStyle, &Value),
}

fn real(v: &Value) -> Option<f64> {
    match v {
        Value::Real(r) => Some(*r),
        Value::Int(i) => Some(*i as f64),
        Value::Text(_) => None,
    }
}

fn int(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(*i),
        Value::Real(r) if r.fract() == 0.0 => Some(*r as i64),
        _ => None,
    }
}

fn flag(on: bool) -> Value {
    Value::Int(i64::from(on))
}

/// A table of one variable per line: its group code, how to read it from the
/// style and how to put a value into it.
macro_rules! entries {
    ($( $code:literal => |$g:ident| $get:expr, |$s:ident, $v:ident| $set:expr; )*) => {
        const ENTRIES: &[Entry] = &[
            $( Entry {
                code: $code,
                get: |$g| $get,
                set: |$s, $v| $set,
            }, )*
        ];
    };
}

entries! {
    3 => |d| Value::Text(d.post.clone()), |s, v| if let Value::Text(t) = v { s.post = t.clone() };
    5 => |d| Value::Text(
        match d.arrow {
            Arrow::Open => "_OPEN",
            Arrow::None => "_NONE",
            _ => "",
        }
        .to_owned()
    ), |s, v| if let Value::Text(t) = v {
        match t.to_uppercase().as_str() {
            "_OPEN" => s.arrow = Arrow::Open,
            "_NONE" => s.arrow = Arrow::None,
            "_ARCHTICK" | "_OBLIQUE" => s.arrow = Arrow::Tick,
            _ => {}
        }
    };
    40 => |d| Value::Real(d.scale), |s, v| if let Some(r) = real(v) { s.scale = r };
    41 => |d| Value::Real(d.arrow_size), |s, v| if let Some(r) = real(v) { s.arrow_size = r };
    42 => |d| Value::Real(d.ext_offset), |s, v| if let Some(r) = real(v) { s.ext_offset = r };
    43 => |d| Value::Real(d.baseline_spacing), |s, v| if let Some(r) = real(v) { s.baseline_spacing = r };
    44 => |d| Value::Real(d.ext_extend), |s, v| if let Some(r) = real(v) { s.ext_extend = r };
    45 => |d| Value::Real(d.round_to), |s, v| if let Some(r) = real(v) { s.round_to = r };
    75 => |d| flag(d.suppress_ext[0]), |s, v| if let Some(i) = int(v) { s.suppress_ext[0] = i != 0 };
    76 => |d| flag(d.suppress_ext[1]), |s, v| if let Some(i) = int(v) { s.suppress_ext[1] = i != 0 };
    73 => |d| flag(d.inside_horizontal), |s, v| if let Some(i) = int(v) { s.inside_horizontal = i != 0 };
    74 => |d| flag(d.outside_horizontal), |s, v| if let Some(i) = int(v) { s.outside_horizontal = i != 0 };
    77 => |d| Value::Int(i64::from(d.text_above)), |s, v| if let Some(i) = int(v) { s.text_above = i.clamp(0, 4) as u8 };
    78 => |d| Value::Int(if d.trim_zeros { 8 } else { 0 }), |s, v| if let Some(i) = int(v) { s.trim_zeros = i & 8 != 0 };
    90 => |d| Value::Int(i64::from(d.arc_symbol)), |s, v| if let Some(i) = int(v) { s.arc_symbol = i.clamp(0, 2) as u8 };
    140 => |d| Value::Real(d.text_height), |s, v| if let Some(r) = real(v) { s.text_height = r };
    141 => |d| Value::Real(d.centre_mark), |s, v| if let Some(r) = real(v) { s.centre_mark = r };
    142 => |d| Value::Real(if d.arrow == Arrow::Tick { d.arrow_size } else { 0.0 }),
        |s, v| if let Some(r) = real(v) { if r > 0.0 { s.arrow = Arrow::Tick; s.arrow_size = r } };
    144 => |d| Value::Real(d.linear_factor), |s, v| if let Some(r) = real(v) { s.linear_factor = r };
    147 => |d| Value::Real(d.text_gap), |s, v| if let Some(r) = real(v) { s.text_gap = r };
    172 => |d| flag(d.force_line), |s, v| if let Some(i) = int(v) { s.force_line = i != 0 };
    174 => |d| flag(d.text_inside), |s, v| if let Some(i) = int(v) { s.text_inside = i != 0 };
    179 => |d| Value::Int(i64::from(d.angle_precision)), |s, v| if let Some(i) = int(v) { s.angle_precision = i.clamp(0, 8) as u8 };
    271 => |d| Value::Int(i64::from(d.precision)), |s, v| if let Some(i) = int(v) { s.precision = i.clamp(0, 8) as u8 };
    275 => |d| Value::Int(d.angle_format.code()),
        |s, v| if let Some(f) = int(v).and_then(AngleFormat::from_code) { s.angle_format = f };
    277 => |d| Value::Int(d.length_format.code()),
        |s, v| if let Some(f) = int(v).and_then(LengthFormat::from_code) { s.length_format = f };
    278 => |d| Value::Int(i64::from(u32::from(d.decimal_separator))),
        |s, v| if let Some(c) = int(v).and_then(|i| u32::try_from(i).ok()).and_then(char::from_u32) {
            if c != '\0' { s.decimal_separator = c }
        };
    279 => |d| Value::Int(i64::from(d.text_move)), |s, v| if let Some(i) = int(v) { s.text_move = i.clamp(0, 2) as u8 };
    280 => |d| Value::Int(i64::from(d.text_just)), |s, v| if let Some(i) = int(v) { s.text_just = i.clamp(0, 4) as u8 };
    281 => |d| flag(d.suppress_line[0]), |s, v| if let Some(i) = int(v) { s.suppress_line[0] = i != 0 };
    282 => |d| flag(d.suppress_line[1]), |s, v| if let Some(i) = int(v) { s.suppress_line[1] = i != 0 };
    289 => |d| Value::Int(i64::from(d.fit)), |s, v| if let Some(i) = int(v) { s.fit = i.clamp(0, 3) as u8 };
}

/// The arrowhead name (code 5) is only read: in a file after R12 AutoCAD
/// keeps arrowheads as block handles, and a name under code 5 would be
/// taken for a handle.
fn written(entry: &&Entry) -> bool {
    entry.code != 5
}

/// Every variable of `style` as (group code, value).
pub(super) fn codes(style: &DimStyle) -> Vec<(i32, Value)> {
    ENTRIES
        .iter()
        .filter(written)
        .map(|e| (e.code, (e.get)(style)))
        .collect()
}

/// The variables in which `style` differs from `base`: what a dimension
/// overrides.
pub(super) fn differences(style: &DimStyle, base: &DimStyle) -> Vec<(i32, Value)> {
    ENTRIES
        .iter()
        .filter(written)
        .filter_map(|e| {
            let (mine, theirs) = ((e.get)(style), (e.get)(base));
            (mine != theirs).then_some((e.code, mine))
        })
        .collect()
}

/// Put a value read under `code` into `style`; false for a code that is not
/// one of the variables kept.
pub(super) fn apply(style: &mut DimStyle, code: i32, value: &Value) -> bool {
    match ENTRIES.iter().find(|e| e.code == code) {
        Some(entry) => {
            (entry.set)(style, value);
            true
        }
        None => false,
    }
}

/// The text of a variable as a group value.
pub(super) fn text_of(value: &Value) -> String {
    match value {
        Value::Real(r) => r.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Text(t) => t.clone(),
    }
}

/// The value group in extended data for a variable: 1040 real, 1070 integer
/// or 1000 string.
pub(super) fn xdata_code(value: &Value) -> i32 {
    match value {
        Value::Real(_) => 1040,
        Value::Int(_) => 1070,
        Value::Text(_) => 1000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom() -> DimStyle {
        DimStyle {
            scale: 2.0,
            arrow: Arrow::Tick,
            arrow_size: 1.5,
            text_height: 3.5,
            text_gap: 0.9,
            text_above: 0,
            text_just: 4,
            text_move: 2,
            fit: 1,
            precision: 4,
            trim_zeros: true,
            decimal_separator: ',',
            post: "<> mm".to_owned(),
            length_format: LengthFormat::Architectural,
            angle_format: AngleFormat::Radians,
            angle_precision: 3,
            suppress_ext: [true, false],
            suppress_line: [false, true],
            centre_mark: -2.0,
            baseline_spacing: 7.0,
            linear_factor: 25.4,
            round_to: 0.5,
            ..DimStyle::default()
        }
    }

    #[test]
    fn every_variable_survives_a_round_trip() {
        let wanted = custom();
        let mut back = DimStyle::default();
        for (code, value) in codes(&wanted) {
            assert!(apply(&mut back, code, &value), "{code}");
        }
        assert_eq!(back, wanted);
    }

    #[test]
    fn only_the_differences_are_listed() {
        let base = DimStyle::default();
        assert!(differences(&base, &base).is_empty());
        let mut tall = base.clone();
        tall.text_height = 5.0;
        assert_eq!(differences(&tall, &base), [(140, Value::Real(5.0))]);
    }

    #[test]
    fn open_arrows_and_unknown_codes() {
        let mut s = DimStyle::default();
        assert!(apply(&mut s, 5, &Value::Text("_OPEN".to_owned())));
        assert_eq!(s.arrow, Arrow::Open);
        assert!(!apply(&mut s, 9999, &Value::Int(1)));
        // A wrong kind of value changes nothing.
        let before = s.clone();
        apply(&mut s, 140, &Value::Text("x".to_owned()));
        assert_eq!(s, before);
    }
}
