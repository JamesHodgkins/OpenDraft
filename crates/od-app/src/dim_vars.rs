//! The dimension style variables, with AutoCAD's names and values. They
//! change the style new dimensions are made with, which is saved in the
//! drawing, so setting one is an undoable change. Dimensions already drawn
//! keep the style they were made with.

use od_commands::{SetDimStyle, SysVar, Value};
use od_core::{Arrow, DimStyle, LengthFormat};

use crate::state::AppState;

impl AppState {
    /// The style new dimensions are made with.
    pub fn dim_style(&self) -> DimStyle {
        self.session().document().dim_style().clone()
    }

    /// Change the current dimension style as one undo step. Nothing is done
    /// when it is already this.
    pub fn set_dim_style(&mut self, style: DimStyle) {
        let style = style.sanitized();
        if style == self.dim_style() {
            return;
        }
        self.perform(SetDimStyle::new(style));
        self.message = "Dimension style changed".to_owned();
    }
}

fn int(value: Value) -> i64 {
    match value {
        Value::Int(n) => n,
        _ => 0,
    }
}

fn real(value: Value) -> f64 {
    match value {
        Value::Real(n) => n,
        _ => 0.0,
    }
}

fn text(value: Value) -> String {
    match value {
        Value::Text(t) => t,
        _ => String::new(),
    }
}

fn flag(on: bool) -> Value {
    Value::Int(i64::from(on))
}

fn change(s: &mut AppState, edit: impl FnOnce(&mut DimStyle)) -> Result<(), String> {
    let mut style = s.dim_style();
    edit(&mut style);
    s.set_dim_style(style);
    Ok(())
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::real(
            "DIMSCALE",
            "Overall scale of dimension sizes (text, arrows, gaps); never the measured value",
            (1.0e-6, 1.0e6),
            |s| Value::Real(s.dim_style().scale),
            |s, v| change(s, |d| d.scale = real(v)),
        ),
        SysVar::real(
            "DIMASZ",
            "Arrow size",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().arrow_size),
            |s, v| change(s, |d| d.arrow_size = real(v)),
        ),
        SysVar::real(
            "DIMTSZ",
            "Size of architectural ticks drawn in place of arrows; 0 for arrows",
            (0.0, 1.0e9),
            |s| {
                let d = s.dim_style();
                Value::Real(if d.arrow == Arrow::Tick { d.arrow_size } else { 0.0 })
            },
            |s, v| {
                let tick = real(v);
                change(s, |d| {
                    if tick > 0.0 {
                        d.arrow = Arrow::Tick;
                        d.arrow_size = tick;
                    } else if d.arrow == Arrow::Tick {
                        d.arrow = Arrow::Closed;
                    }
                })
            },
        ),
        SysVar::real(
            "DIMEXO",
            "Gap between the measured point and the extension line",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().ext_offset),
            |s, v| change(s, |d| d.ext_offset = real(v)),
        ),
        SysVar::real(
            "DIMEXE",
            "How far the extension line runs past the dimension line",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().ext_extend),
            |s, v| change(s, |d| d.ext_extend = real(v)),
        ),
        SysVar::real(
            "DIMTXT",
            "Text height",
            (1.0e-6, 1.0e9),
            |s| Value::Real(s.dim_style().text_height),
            |s, v| change(s, |d| d.text_height = real(v)),
        ),
        SysVar::real(
            "DIMGAP",
            "Clearance around the text",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().text_gap),
            |s, v| change(s, |d| d.text_gap = real(v)),
        ),
        SysVar::real(
            "DIMCEN",
            "Size of the centre mark of radius and diameter dimensions; 0 for none",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().centre_mark),
            |s, v| change(s, |d| d.centre_mark = real(v)),
        ),
        SysVar::int(
            "DIMTAD",
            "Text against the line: 0 centred, 1 above, 2 outside, 3 JIS, 4 below",
            (0, 4),
            |s| Value::Int(i64::from(s.dim_style().text_above)),
            |s, v| change(s, |d| d.text_above = int(v) as u8),
        ),
        SysVar::int(
            "DIMJUST",
            "Text along the line: 0 centred, 1 at extension line 1, 2 at extension line 2",
            (0, 2),
            |s| Value::Int(i64::from(s.dim_style().text_just)),
            |s, v| change(s, |d| d.text_just = int(v) as u8),
        ),
        SysVar::int(
            "DIMTIH",
            "Text inside the extension lines: 0 aligned with the line, 1 horizontal",
            (0, 1),
            |s| flag(s.dim_style().inside_horizontal),
            |s, v| change(s, |d| d.inside_horizontal = int(v) == 1),
        ),
        SysVar::int(
            "DIMTOH",
            "Text outside the extension lines: 0 aligned with the line, 1 horizontal",
            (0, 1),
            |s| flag(s.dim_style().outside_horizontal),
            |s, v| change(s, |d| d.outside_horizontal = int(v) == 1),
        ),
        SysVar::int(
            "DIMTIX",
            "Always put text between the extension lines: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().text_inside),
            |s, v| change(s, |d| d.text_inside = int(v) == 1),
        ),
        SysVar::int(
            "DIMTOFL",
            "Draw a line between the extension lines even when text is outside: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().force_line),
            |s, v| change(s, |d| d.force_line = int(v) == 1),
        ),
        SysVar::int(
            "DIMATFIT",
            "When text and arrows do not fit: 0 move both, 1 arrows first, 2 text first, 3 best fit",
            (0, 3),
            |s| Value::Int(i64::from(s.dim_style().fit)),
            |s, v| change(s, |d| d.fit = int(v) as u8),
        ),
        SysVar::int(
            "DIMTMOVE",
            "Moved text: 0 moves the dimension line, 1 adds a leader, 2 moves freely",
            (0, 2),
            |s| Value::Int(i64::from(s.dim_style().text_move)),
            |s, v| change(s, |d| d.text_move = int(v) as u8),
        ),
        SysVar::int(
            "DIMSD1",
            "Suppress the first half of the dimension line: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().suppress_line[0]),
            |s, v| change(s, |d| d.suppress_line[0] = int(v) == 1),
        ),
        SysVar::int(
            "DIMSD2",
            "Suppress the second half of the dimension line: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().suppress_line[1]),
            |s, v| change(s, |d| d.suppress_line[1] = int(v) == 1),
        ),
        SysVar::int(
            "DIMSE1",
            "Suppress the first extension line: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().suppress_ext[0]),
            |s, v| change(s, |d| d.suppress_ext[0] = int(v) == 1),
        ),
        SysVar::int(
            "DIMSE2",
            "Suppress the second extension line: 0 off, 1 on",
            (0, 1),
            |s| flag(s.dim_style().suppress_ext[1]),
            |s, v| change(s, |d| d.suppress_ext[1] = int(v) == 1),
        ),
        SysVar::real(
            "DIMLFAC",
            "Scale factor applied to measured lengths for display (not zero)",
            (-1.0e6, 1.0e6),
            |s| Value::Real(s.dim_style().linear_factor),
            |s, v| {
                let factor = real(v);
                if factor == 0.0 {
                    return Err("Requires a value other than zero.".to_owned());
                }
                change(s, |d| d.linear_factor = factor)
            },
        ),
        SysVar::real(
            "DIMRND",
            "Round displayed lengths to a multiple of this; 0 for none",
            (0.0, 1.0e9),
            |s| Value::Real(s.dim_style().round_to),
            |s, v| change(s, |d| d.round_to = real(v)),
        ),
        SysVar::int(
            "DIMLUNIT",
            "Length format: 1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional",
            (1, 5),
            |s| Value::Int(s.dim_style().length_format.code()),
            |s, v| {
                let format = LengthFormat::from_code(int(v)).unwrap_or_default();
                change(s, |d| d.length_format = format)
            },
        ),
        SysVar::int(
            "DIMDEC",
            "Decimal places of a dimension (or the fraction step)",
            (0, 8),
            |s| Value::Int(i64::from(s.dim_style().precision)),
            |s, v| change(s, |d| d.precision = int(v) as u8),
        ),
        SysVar::int(
            "DIMZIN",
            "Zero suppression: 0 keep trailing zeros, 8 drop them",
            (0, 8),
            |s| Value::Int(if s.dim_style().trim_zeros { 8 } else { 0 }),
            |s, v| change(s, |d| d.trim_zeros = int(v) & 8 != 0),
        ),
        SysVar {
            name: "DIMDSEP",
            description: "Decimal separator: one character",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| Value::Text(s.dim_style().decimal_separator.to_string()),
            set: |s, v| {
                let mut chars = text(v).chars().collect::<Vec<_>>();
                match (chars.pop(), chars.is_empty()) {
                    (Some(c), true) => change(s, |d| d.decimal_separator = c),
                    _ => Err("Requires a single character.".to_owned()),
                }
            },
        },
        SysVar {
            name: "DIMPOST",
            description: "Text around the value, <> standing for it (\"<> mm\"); . for none",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| Value::Text(s.dim_style().post),
            set: |s, v| {
                let post = text(v);
                change(s, |d| d.post = if post == "." { String::new() } else { post })
            },
        },
        SysVar {
            name: "DIMTXSTY",
            description: "Text style of dimension text; the style must already exist",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| {
                let id = s.dim_style().text_style;
                let name = s.session().document().text_style(id).map(|t| t.name.clone());
                Value::Text(name.unwrap_or_else(|| "Standard".to_owned()))
            },
            set: |s, v| {
                let name = text(v);
                let found = s
                    .session()
                    .document()
                    .text_style_named(&name)
                    .map(|t| t.id);
                let Some(id) = found else {
                    return Err(format!("There is no text style named {name}."));
                };
                change(s, |d| d.text_style = id)
            },
        },
        SysVar {
            name: "DIMBLK",
            description: "Arrowhead: . closed filled, _OPEN, _ARCHTICK, _NONE",
            kind: od_commands::Kind::Text,
            read_only: false,
            get: |s| {
                Value::Text(
                    match s.dim_style().arrow {
                        Arrow::Closed => ".",
                        Arrow::Open => "_OPEN",
                        Arrow::Tick => "_ARCHTICK",
                        Arrow::None => "_NONE",
                    }
                    .to_owned(),
                )
            },
            set: |s, v| {
                let arrow = match text(v).to_uppercase().as_str() {
                    "." | "" | "_CLOSEDFILLED" => Arrow::Closed,
                    "_OPEN" => Arrow::Open,
                    "_ARCHTICK" => Arrow::Tick,
                    "_NONE" => Arrow::None,
                    _ => return Err("Use . _OPEN _ARCHTICK or _NONE.".to_owned()),
                };
                change(s, |d| d.arrow = arrow)
            },
        },
        SysVar::int(
            "DIMARCSYM",
            "Arc length symbol: 0 before the text, 1 above it, 2 none",
            (0, 2),
            |s| Value::Int(i64::from(s.dim_style().arc_symbol)),
            |s, v| change(s, |d| d.arc_symbol = int(v) as u8),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::sysvars::table;

    use super::*;

    #[test]
    fn the_variables_read_and_set_the_current_style() {
        let (table, mut state) = (table(), AppState::new());
        assert_eq!(table.get("DIMTXT", &state), Ok(Value::Real(2.5)));
        table.set("dimtxt", "4", &mut state).unwrap();
        table.set("DIMASZ", "3", &mut state).unwrap();
        table.set("DIMTAD", "0", &mut state).unwrap();
        table.set("DIMLUNIT", "4", &mut state).unwrap();
        table.set("DIMPOST", "<> mm", &mut state).unwrap();
        table.set("DIMBLK", "_OPEN", &mut state).unwrap();
        assert!(table.set("DIMTXSTY", "Nope", &mut state).is_err());
        let style = state.dim_style();
        assert_eq!(style.text_height, 4.0);
        assert_eq!(style.arrow_size, 3.0);
        assert_eq!(style.text_above, 0);
        assert_eq!(style.length_format, LengthFormat::Architectural);
        assert_eq!(style.post, "<> mm");
        assert_eq!(style.arrow, Arrow::Open);
        assert!(state.session().is_modified(), "saved in the drawing");
    }

    #[test]
    fn a_change_is_one_undo_step() {
        let (table, mut state) = (table(), AppState::new());
        table.set("DIMTXT", "5", &mut state).unwrap();
        state.undo();
        assert_eq!(state.dim_style().text_height, 2.5);
    }

    #[test]
    fn bad_values_are_refused() {
        let (table, mut state) = (table(), AppState::new());
        assert!(table.set("DIMTAD", "9", &mut state).is_err());
        assert!(table.set("DIMLFAC", "0", &mut state).is_err());
        assert!(table.set("DIMDSEP", "ab", &mut state).is_err());
        assert!(table.set("DIMBLK", "nonsense", &mut state).is_err());
        assert_eq!(state.dim_style(), DimStyle::default());
    }

    #[test]
    fn ticks_replace_arrows() {
        let (table, mut state) = (table(), AppState::new());
        table.set("DIMTSZ", "1.5", &mut state).unwrap();
        assert_eq!(state.dim_style().arrow, Arrow::Tick);
        assert_eq!(table.get("DIMTSZ", &state), Ok(Value::Real(1.5)));
        table.set("DIMTSZ", "0", &mut state).unwrap();
        assert_eq!(state.dim_style().arrow, Arrow::Closed);
    }
}
