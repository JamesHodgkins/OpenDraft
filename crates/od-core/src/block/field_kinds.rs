//! The fields a user can ask for, as the Field dialog lists them: a name, a
//! category, and the formats that suit it, with the date and text-case
//! formatting the evaluators use. Codes follow AutoCAD's: `\AcVar Date
//! \f "M/d/yyyy"`, and `%tc1` to `%tc4` for upper, lower, first capital and
//! title case.

use super::field::DataField;
use super::field_eval::{FieldContext, VARIABLE};

/// A date and time, as the pieces a pattern picks from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldTime {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    pub day: u8,
    /// 0 for Monday to 6 for Sunday.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// What `Date` shows when its field names no format.
pub const DEFAULT_DATE: &str = "M/d/yyyy";

/// The date formats offered, as patterns: `yyyy` `yy` year, `MMMM` `MMM` `MM`
/// `M` month, `dddd` `ddd` `dd` `d` day, `HH` `H` 24-hour and `hh` `h`
/// 12-hour clock, `mm` `m` minutes, `ss` `s` seconds, `tt` AM or PM; text in
/// single quotes is kept as it is.
const DATE_FORMATS: [&str; 10] = [
    "M/d/yyyy",
    "dddd, MMMM d, yyyy",
    "MMMM d, yyyy",
    "MMM d, yyyy",
    "d MMMM yyyy",
    "dd/MM/yyyy",
    "yyyy-MM-dd",
    "h:mm tt",
    "HH:mm",
    "yyyy-MM-dd HH:mm",
];

/// `time` written as `pattern`.
pub fn format_time(time: &FieldTime, pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|x| **x == c).count();
        i += run;
        let two = |v: u8| format!("{v:02}");
        let hour12 = if time.hour.is_multiple_of(12) {
            12
        } else {
            time.hour % 12
        };
        let month = MONTHS[usize::from(time.month.clamp(1, 12)) - 1];
        let day = DAYS[usize::from(time.weekday.min(6))];
        match (c, run) {
            ('y', 1..=2) => out.push_str(&two(time.year.rem_euclid(100) as u8)),
            ('y', _) => out.push_str(&format!("{:04}", time.year)),
            ('M', 1) => out.push_str(&time.month.to_string()),
            ('M', 2) => out.push_str(&two(time.month)),
            ('M', 3) => out.push_str(&month[..3]),
            ('M', _) => out.push_str(month),
            ('d', 1) => out.push_str(&time.day.to_string()),
            ('d', 2) => out.push_str(&two(time.day)),
            ('d', 3) => out.push_str(&day[..3]),
            ('d', _) => out.push_str(day),
            ('H', 1) => out.push_str(&time.hour.to_string()),
            ('H', _) => out.push_str(&two(time.hour)),
            ('h', 1) => out.push_str(&hour12.to_string()),
            ('h', _) => out.push_str(&two(hour12)),
            ('m', 1) => out.push_str(&time.minute.to_string()),
            ('m', _) => out.push_str(&two(time.minute)),
            ('s', 1) => out.push_str(&time.second.to_string()),
            ('s', _) => out.push_str(&two(time.second)),
            ('t', _) => out.push_str(if time.hour < 12 { "AM" } else { "PM" }),
            (other, n) => out.extend(std::iter::repeat_n(other, n)),
        }
    }
    out
}

/// The text-case formats: label and code.
const CASES: [(&str, &str); 4] = [
    ("Uppercase", "%tc1"),
    ("Lowercase", "%tc2"),
    ("First capital", "%tc3"),
    ("Title case", "%tc4"),
];

/// `text` in the case `code` names; any other code leaves it alone.
pub fn apply_case(text: &str, code: &str) -> String {
    match code {
        "%tc1" => text.to_uppercase(),
        "%tc2" => text.to_lowercase(),
        "%tc3" => {
            let mut chars = text.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect()
            })
        }
        "%tc4" => {
            let mut out = String::with_capacity(text.len());
            let mut start = true;
            for c in text.chars() {
                if c.is_alphanumeric() {
                    if start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start = false;
                } else {
                    out.push(c);
                    start = true;
                }
            }
            out
        }
        _ => text.to_string(),
    }
}

/// Where a field is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCategory {
    DateTime,
    Document,
    Objects,
}

impl FieldCategory {
    pub const ALL: [FieldCategory; 3] = [Self::DateTime, Self::Document, Self::Objects];

    pub fn label(self) -> &'static str {
        match self {
            Self::DateTime => "Date & Time",
            Self::Document => "Document",
            Self::Objects => "Objects",
        }
    }
}

/// A field the dialog lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldKind {
    pub name: &'static str,
    pub category: FieldCategory,
}

/// The fields offered, in the order the dialog lists them.
/// The field that shows a property of an object in the drawing; it is made
/// from the object picked, not from a format.
pub const OBJECT: &str = "Object";

pub const KINDS: [FieldKind; 9] = [
    FieldKind {
        name: "Author",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: "Comments",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: "Date",
        category: FieldCategory::DateTime,
    },
    FieldKind {
        name: "Filename",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: "HyperlinkBase",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: "Keywords",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: OBJECT,
        category: FieldCategory::Objects,
    },
    FieldKind {
        name: "Subject",
        category: FieldCategory::Document,
    },
    FieldKind {
        name: "Title",
        category: FieldCategory::Document,
    },
];

/// One entry of the Format list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatChoice {
    /// What the list shows: the name of a case, or a date as it would read now.
    pub label: String,
    /// The code in the field: empty for `(none)`.
    pub code: String,
}

/// The formats that suit the field `name`, `(none)` first.
pub fn formats(name: &str, context: &FieldContext) -> Vec<FormatChoice> {
    let none = FormatChoice {
        label: "(none)".into(),
        code: String::new(),
    };
    let rest: Vec<FormatChoice> = match name {
        "Date" => DATE_FORMATS
            .iter()
            .map(|p| FormatChoice {
                label: context
                    .now
                    .map_or_else(|| (*p).to_string(), |t| format_time(&t, p)),
                code: (*p).to_string(),
            })
            .collect(),
        // Every other field shows text, which can be put in a case.
        n if n != OBJECT => CASES
            .iter()
            .map(|(label, code)| FormatChoice {
                label: (*label).into(),
                code: (*code).into(),
            })
            .collect(),
        _ => Vec::new(),
    };
    std::iter::once(none).chain(rest).collect()
}

impl DataField {
    /// A drawing variable (`\AcVar name`) with a format code, or none.
    pub fn variable(name: &str, format: &str) -> Self {
        let mut code = format!("\\{VARIABLE} {name}");
        if !format.is_empty() {
            code.push_str(&format!(" \\f \"{format}\""));
        }
        let mut field = Self::new(VARIABLE, &code, None);
        field.format = format.to_string();
        field.status = super::field::FieldStatus::Unevaluated;
        field
    }

    /// The field `name` of the dialog's list with `format`, `None` if no such
    /// field is offered.
    pub fn of_kind(name: &str, format: &str) -> Option<Self> {
        KINDS
            .iter()
            .any(|k| k.name == name && name != OBJECT)
            .then(|| Self::variable(name, format))
    }

    /// The format code in a field's code (`\f "..."`), if it has one.
    pub fn format_code(&self) -> Option<&str> {
        let (_, after) = self.code.split_once("\\f \"")?;
        after.split('"').next()
    }

    /// The field as AutoCAD writes it, `%<\AcVar Date \f "M/d/yyyy">%`, for
    /// the dialog's expression box.
    pub fn expression(&self) -> String {
        let code = match self.object_refs.first() {
            Some(super::field::FieldObjectRef::Entity(id)) => self
                .code
                .replace("%<\\_ObjId 0>%", &format!("%<\\_ObjId {}>%", id.0)),
            _ => self.code.clone(),
        };
        format!("%<{code}>%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: FieldTime = FieldTime {
        year: 2026,
        month: 11,
        day: 1,
        weekday: 6,
        hour: 15,
        minute: 7,
        second: 9,
    };

    #[test]
    fn dates_follow_their_patterns() {
        let f = |p| format_time(&T, p);
        assert_eq!(f("M/d/yyyy"), "11/1/2026");
        assert_eq!(f("dddd, MMMM d, yyyy"), "Sunday, November 1, 2026");
        assert_eq!(f("MMM d, yy"), "Nov 1, 26");
        assert_eq!(f("dd/MM/yyyy"), "01/11/2026");
        assert_eq!(f("h:mm tt"), "3:07 PM");
        assert_eq!(f("HH:mm:ss"), "15:07:09");
        assert_eq!(f("'on' ddd"), "on Sun");
    }

    #[test]
    fn midnight_and_noon_read_as_twelve() {
        let at = |hour| FieldTime { hour, ..T };
        assert_eq!(format_time(&at(0), "h tt"), "12 AM");
        assert_eq!(format_time(&at(12), "h tt"), "12 PM");
    }

    #[test]
    fn cases_follow_their_codes() {
        assert_eq!(apply_case("my PLAN.odx", "%tc1"), "MY PLAN.ODX");
        assert_eq!(apply_case("my PLAN.odx", "%tc2"), "my plan.odx");
        assert_eq!(apply_case("my PLAN.odx", "%tc3"), "My plan.odx");
        assert_eq!(apply_case("my PLAN.odx", "%tc4"), "My Plan.Odx");
        assert_eq!(apply_case("Same", ""), "Same");
    }

    #[test]
    fn a_variable_carries_its_format_in_its_code_and_expression() {
        let f = DataField::variable("Date", "yyyy-MM-dd");
        assert_eq!(f.expression(), "%<\\AcVar Date \\f \"yyyy-MM-dd\">%");
        assert_eq!(f.format_code(), Some("yyyy-MM-dd"));
        let plain = DataField::variable("Filename", "");
        assert_eq!(plain.expression(), "%<\\AcVar Filename>%");
        assert_eq!(plain.format_code(), None);
    }

    #[test]
    fn the_format_list_starts_with_none_and_dates_show_the_time_now() {
        let ctx = FieldContext {
            now: Some(T),
            ..FieldContext::default()
        };
        let dates = formats("Date", &ctx);
        assert_eq!(dates[0].label, "(none)");
        assert_eq!(dates[1].label, "11/1/2026");
        assert_eq!(dates[1].code, "M/d/yyyy");
        assert_eq!(formats("Filename", &ctx).len(), 5);
        assert!(DataField::of_kind("Nope", "").is_none());
    }
}
