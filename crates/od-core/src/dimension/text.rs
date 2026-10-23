//! The words on a dimension: the measured value written in the style's
//! units, with prefix, suffix and any text the user typed over it.

use super::style::DimStyle;
use crate::units::{format_length, LengthFormat};

/// What a dimension shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum DimText {
    /// The measurement.
    #[default]
    Measured,
    /// No text at all (AutoCAD: a single space).
    Hidden,
    /// Typed text; the first `<>` stands for the measurement.
    Custom(String),
}

impl DimText {
    /// How AutoCAD files it: empty or `<>` is the measurement, one space
    /// hides the text, anything else is the text.
    pub fn from_entry(entry: &str) -> DimText {
        match entry {
            "" | "<>" => DimText::Measured,
            " " => DimText::Hidden,
            other => DimText::Custom(other.to_owned()),
        }
    }

    pub fn to_entry(&self) -> String {
        match self {
            DimText::Measured => String::new(),
            DimText::Hidden => " ".to_owned(),
            DimText::Custom(text) => text.clone(),
        }
    }
}

/// `%%c`, `%%d` and `%%p` become the diameter, degree and plus-minus signs;
/// `%%%` a percent sign.
pub fn expand_codes(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("%%") {
        out.push_str(&rest[..i]);
        let code = rest[i + 2..].chars().next();
        let symbol = match code {
            Some('c' | 'C') => 'Ø',
            Some('d' | 'D') => '°',
            Some('p' | 'P') => '±',
            Some('%') => '%',
            _ => {
                out.push_str("%%");
                rest = &rest[i + 2..];
                continue;
            }
        };
        out.push(symbol);
        rest = &rest[i + 2 + 1..];
    }
    out.push_str(rest);
    out
}

/// The measured `value` as the style writes it: scaled by `DIMLFAC`,
/// rounded by `DIMRND`, formatted in the style's units with its zero
/// suppression and decimal separator, then wrapped in `DIMPOST`.
/// `prefix` is the radius or diameter sign.
pub fn measurement_text(style: &DimStyle, value: f64, prefix: &str) -> String {
    let mut v = value * style.linear_factor;
    if style.round_to > 0.0 {
        v = (v / style.round_to).round() * style.round_to;
    }
    let mut shown = format_length(&style.units(), v);
    if style.length_format == LengthFormat::Decimal {
        if style.trim_zeros && shown.contains('.') {
            shown = shown.trim_end_matches('0').trim_end_matches('.').to_owned();
        }
        if style.decimal_separator != '.' {
            shown = shown.replace('.', &style.decimal_separator.to_string());
        }
    }
    let core = format!("{prefix}{shown}");
    if style.post.is_empty() {
        core
    } else if style.post.contains("<>") {
        expand_codes(&style.post.replacen("<>", &core, 1))
    } else {
        format!("{core}{}", expand_codes(&style.post))
    }
}

/// What to print for `text` given the generated `measured` string.
pub fn shown(text: &DimText, measured: &str) -> String {
    match text {
        DimText::Measured => measured.to_owned(),
        DimText::Hidden => String::new(),
        DimText::Custom(typed) => expand_codes(&typed.replacen("<>", measured, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_uses_precision() {
        let style = DimStyle::default();
        assert_eq!(measurement_text(&style, 12.3456, ""), "12.35");
    }

    #[test]
    fn linear_factor_scales_and_rounding_snaps() {
        let style = DimStyle {
            linear_factor: 2.0,
            round_to: 0.5,
            precision: 1,
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&style, 3.3, ""), "6.5");
    }

    #[test]
    fn trailing_zeros_can_be_dropped() {
        let style = DimStyle {
            trim_zeros: true,
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&style, 5.0, ""), "5");
        assert_eq!(measurement_text(&style, 5.5, ""), "5.5");
    }

    #[test]
    fn decimal_separator_is_swapped() {
        let style = DimStyle {
            decimal_separator: ',',
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&style, 1.5, ""), "1,50");
    }

    #[test]
    fn post_wraps_the_value() {
        let style = DimStyle {
            post: "<> mm".to_owned(),
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&style, 10.0, ""), "10.00 mm");
        let suffix = DimStyle {
            post: "mm".to_owned(),
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&suffix, 10.0, "R"), "R10.00mm");
    }

    #[test]
    fn architectural_units_are_honoured() {
        let style = DimStyle {
            length_format: LengthFormat::Architectural,
            precision: 2,
            ..DimStyle::default()
        };
        assert_eq!(measurement_text(&style, 18.0, ""), "1'-6\"");
    }

    #[test]
    fn typed_text_replaces_only_the_first_marker() {
        let t = DimText::Custom("<> TYP <>".to_owned());
        assert_eq!(shown(&t, "5"), "5 TYP <>");
        assert_eq!(shown(&DimText::Hidden, "5"), "");
        assert_eq!(shown(&DimText::Measured, "5"), "5");
    }

    #[test]
    fn control_codes_expand() {
        assert_eq!(expand_codes("%%c10 %%p1 45%%d 5%%%"), "Ø10 ±1 45° 5%");
        assert_eq!(expand_codes("a%%xb"), "a%%xb");
    }

    #[test]
    fn entry_strings_follow_autocad() {
        assert_eq!(DimText::from_entry(""), DimText::Measured);
        assert_eq!(DimText::from_entry("<>"), DimText::Measured);
        assert_eq!(DimText::from_entry(" "), DimText::Hidden);
        assert_eq!(DimText::from_entry("x"), DimText::Custom("x".into()));
        assert_eq!(DimText::Hidden.to_entry(), " ");
    }
}
