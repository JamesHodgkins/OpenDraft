//! The kinds of value a Properties row holds, and how each is written and
//! read in the drawing's units.

use od_core::dimension::Arrow;
use od_core::hatch::HatchStyle;
use od_core::mleader::MLeaderLineType;
use od_core::units::{format_angle, format_length, parse_angle, parse_length};
use od_core::{AngleFormat, Units};

use super::Field;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Length,
    Angle,
    Area,
    /// A plain count.
    Count,
    /// A plain number with decimals, such as a scale factor.
    Factor,
    /// Yes or no.
    Flag,
    /// A hatch's fill: pattern, solid or gradient.
    Fill,
    /// A hatch's island style: normal, outer or ignore.
    Style,
    /// A multileader's leader type, arrowhead and content kind.
    LeaderType,
    Arrowhead,
    Content,
}

impl Unit {
    /// What follows a value of this kind in the drawing's units, for tooltips.
    pub fn suffix(self, units: &Units) -> String {
        let name = units.insert.abbreviation();
        match self {
            Unit::Length => name.to_owned(),
            Unit::Angle => match units.angle {
                AngleFormat::DecimalDegrees => "\u{b0}".to_owned(),
                format => format.label().to_lowercase(),
            },
            Unit::Area if name.is_empty() => String::new(),
            Unit::Area => format!("{name}\u{b2}"),
            Unit::Count
            | Unit::Factor
            | Unit::Flag
            | Unit::Fill
            | Unit::Style
            | Unit::LeaderType
            | Unit::Arrowhead
            | Unit::Content => String::new(),
        }
    }
}

impl Field {
    /// What kind of value the field holds.
    pub fn unit(self) -> Unit {
        match self {
            Field::Angle
            | Field::StartAngle
            | Field::EndAngle
            | Field::StartParameter
            | Field::EndParameter => Unit::Angle,
            Field::Area => Unit::Area,
            Field::Vertices
            | Field::Lines
            | Field::Degree
            | Field::ControlVertices
            | Field::Knots
            | Field::FitPoints => Unit::Count,
            Field::Closed | Field::Periodic | Field::Rational => Unit::Flag,
            Field::Double | Field::Associative | Field::Dogleg | Field::Frame => Unit::Flag,
            Field::LeaderType => Unit::LeaderType,
            Field::Arrowhead => Unit::Arrowhead,
            Field::ContentKind => Unit::Content,
            Field::Loops => Unit::Count,
            Field::FillKind => Unit::Fill,
            Field::HatchStyle => Unit::Style,
            Field::ScaleX | Field::ScaleY | Field::RadiusRatio | Field::Scale => Unit::Factor,
            _ => Unit::Length,
        }
    }
}

/// A value as the panel shows and edits it, in the drawing's units.
pub fn format(units: &Units, unit: Unit, value: f64) -> String {
    match unit {
        Unit::Length => format_length(units, value),
        Unit::Angle => format_angle(units, value.to_radians()),
        Unit::Area => format!("{value:.p$}", p = usize::from(units.length_precision)),
        Unit::Count => format!("{value:.0}"),
        Unit::Factor => format!("{value:.p$}", p = usize::from(units.length_precision)),
        Unit::Flag if value != 0.0 => "Yes".to_owned(),
        Unit::Flag => "No".to_owned(),
        Unit::Fill => match value.round() as i64 {
            0 => "Pattern",
            1 => "Solid",
            _ => "Gradient",
        }
        .to_owned(),
        Unit::Style => HatchStyle::from_code(value.round() as i64)
            .map_or("?", HatchStyle::label)
            .to_owned(),
        Unit::LeaderType => MLeaderLineType::ALL
            .get(value.round() as usize)
            .map_or("?", |t| t.label())
            .to_owned(),
        Unit::Arrowhead => Arrow::from_code(value.round() as i64)
            .map_or("?", Arrow::label)
            .to_owned(),
        Unit::Content if value != 0.0 => "MText".to_owned(),
        Unit::Content => "None".to_owned(),
    }
}

/// Parse what the user typed, in the drawing's units. Tolerates surrounding
/// space and the drawing's unit name after a length or area.
pub fn parse(units: &Units, unit: Unit, text: &str) -> Option<f64> {
    let text = text.trim();
    let name = units.insert.abbreviation();
    let without = |mark: &str| match text.strip_suffix(mark) {
        Some(rest) if !mark.is_empty() => rest.trim_end(),
        _ => text,
    };
    let plain = |text: &str| text.parse().ok().filter(|v: &f64| v.is_finite());
    match unit {
        Unit::Length => parse_length(without(name)),
        Unit::Angle => parse_angle(units, text).map(f64::to_degrees),
        Unit::Area => plain(without(&format!("{name}\u{b2}"))),
        Unit::Count | Unit::Factor => plain(text),
        Unit::Flag => match text.to_ascii_lowercase().as_str() {
            "yes" | "y" | "1" => Some(1.0),
            "no" | "n" | "0" => Some(0.0),
            _ => None,
        },
        Unit::Style => match text.to_ascii_lowercase().as_str() {
            "normal" | "n" | "0" => Some(0.0),
            "outer" | "o" | "1" => Some(1.0),
            "ignore" | "i" | "2" => Some(2.0),
            _ => None,
        },
        Unit::LeaderType => MLeaderLineType::ALL
            .iter()
            .position(|t| t.label().eq_ignore_ascii_case(text))
            .map(|i| i as f64)
            .or_else(|| plain(text)),
        Unit::Arrowhead => Arrow::ALL
            .iter()
            .find(|a| a.label().eq_ignore_ascii_case(text))
            .map(|a| a.code() as f64)
            .or_else(|| match text.to_ascii_lowercase().as_str() {
                "closed" => Some(0.0),
                "open" => Some(1.0),
                "tick" => Some(2.0),
                _ => plain(text),
            }),
        // Chosen by HATCH or by the style, not typed.
        Unit::Fill | Unit::Content => None,
    }
}
