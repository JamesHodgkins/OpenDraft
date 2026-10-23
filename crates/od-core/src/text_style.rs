//! Text styles: the named record an `MText` refers to for its font and
//! shape. The table itself lives on the `Document`.

use crate::text::{MAX_WEIGHT, MIN_WEIGHT};

/// Permanent identity of a text style; never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextStyleId(pub u64);

impl TextStyleId {
    /// The style every drawing starts with.
    pub const STANDARD: TextStyleId = TextStyleId(0);
}

/// Longest style name or font string accepted.
pub const MAX_NAME_BYTES: usize = 255;

/// The font a style asks for. What is asked for is kept even when this build
/// has to draw with something else.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontDescriptor {
    /// The font that ships with the application.
    BundledSans,
    TrueType {
        family: String,
        bold: bool,
        italic: bool,
    },
    Shx {
        file_name: String,
    },
    /// OD Roman, the single-stroke font that ships with the application.
    /// Bold thickens its line and italic slants it.
    OdRoman {
        bold: bool,
        italic: bool,
    },
    /// A font record read from a file that this build cannot classify.
    Opaque {
        raw_primary: String,
        raw_typeface: Option<i32>,
    },
}

/// The family the application ships; asking for it by name is the same as
/// asking for the bundled font.
pub const BUNDLED_FAMILY: &str = "DejaVu Sans";

/// The name of the stroke font that ships with the application.
pub const OD_ROMAN_FAMILY: &str = "OD Roman";

/// Pen width, as a fraction of text height, a new style starts with.
pub const DEFAULT_STROKE_WEIGHT: f64 = 0.10;

impl FontDescriptor {
    /// The name a font list shows, and the bold and italic it asks for.
    pub fn name_and_flags(&self) -> (String, bool, bool) {
        match self {
            Self::BundledSans => (BUNDLED_FAMILY.to_owned(), false, false),
            Self::TrueType {
                family,
                bold,
                italic,
            } => (family.clone(), *bold, *italic),
            Self::OdRoman { bold, italic } => (OD_ROMAN_FAMILY.to_owned(), *bold, *italic),
            Self::Shx { file_name } => (file_name.clone(), false, false),
            Self::Opaque { raw_primary, .. } => (raw_primary.clone(), false, false),
        }
    }

    /// The descriptor for a font name as typed: `.shx` is a shape file, the
    /// bundled family in its regular face is the bundled font, anything else
    /// a TrueType family.
    pub fn from_name(name: &str, bold: bool, italic: bool) -> Self {
        let name = name.trim();
        if name.to_ascii_lowercase().ends_with(".shx") {
            Self::Shx {
                file_name: name.to_owned(),
            }
        } else if name.eq_ignore_ascii_case(OD_ROMAN_FAMILY) {
            Self::OdRoman { bold, italic }
        } else if name.eq_ignore_ascii_case(BUNDLED_FAMILY) && !bold && !italic {
            Self::BundledSans
        } else {
            Self::TrueType {
                family: name.to_owned(),
                bold,
                italic,
            }
        }
    }

    pub fn is_shx(&self) -> bool {
        matches!(self, Self::Shx { .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub id: TextStyleId,
    pub name: String,
    pub font: FontDescriptor,
    pub big_font: Option<String>,
    /// A height every use of the style is fixed to; `None` leaves it free.
    pub fixed_height: Option<f64>,
    pub width_factor: f64,
    pub oblique_radians: f64,
    pub backward: bool,
    pub upside_down: bool,
    pub vertical: bool,
    /// The pen width of a stroke font (OD Roman) as a fraction of the text
    /// height; outline fonts ignore it.
    pub stroke_weight: f64,
}

impl TextStyle {
    pub fn new(id: TextStyleId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            font: FontDescriptor::BundledSans,
            big_font: None,
            fixed_height: None,
            width_factor: 1.0,
            oblique_radians: 0.0,
            backward: false,
            upside_down: false,
            vertical: false,
            stroke_weight: DEFAULT_STROKE_WEIGHT,
        }
    }

    pub fn standard() -> Self {
        Self::new(TextStyleId::STANDARD, "Standard")
    }

    /// Whether the style could exist: a bounded name, bounded font strings
    /// and numbers in a sensible range.
    pub fn is_valid(&self) -> bool {
        let bounded = |s: &str| s.len() <= MAX_NAME_BYTES;
        let font_ok = match &self.font {
            FontDescriptor::BundledSans => true,
            FontDescriptor::TrueType { family, .. } => bounded(family),
            FontDescriptor::Shx { file_name } => bounded(file_name),
            FontDescriptor::OdRoman { .. } => true,
            FontDescriptor::Opaque { raw_primary, .. } => bounded(raw_primary),
        };
        !self.name.trim().is_empty()
            && bounded(&self.name)
            && font_ok
            && self.big_font.as_deref().is_none_or(bounded)
            && self.fixed_height.is_none_or(|h| h.is_finite() && h >= 0.0)
            && (MIN_WEIGHT..=MAX_WEIGHT).contains(&self.stroke_weight)
            && self.width_factor.is_finite()
            && (0.01..=100.0).contains(&self.width_factor)
            && self.oblique_radians.is_finite()
            && self.oblique_radians.abs() < std::f64::consts::FRAC_PI_2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_style_is_valid() {
        let standard = TextStyle::standard();
        assert_eq!(standard.id, TextStyleId::STANDARD);
        assert!(standard.is_valid());
    }

    #[test]
    fn font_names_map_to_descriptors_and_back() {
        let bundled = FontDescriptor::from_name(" dejavu sans ", false, false);
        assert_eq!(bundled, FontDescriptor::BundledSans);
        assert_eq!(
            bundled.name_and_flags(),
            ("DejaVu Sans".into(), false, false)
        );
        let bold = FontDescriptor::from_name("DejaVu Sans", true, false);
        assert_eq!(bold.name_and_flags(), ("DejaVu Sans".into(), true, false));
        assert!(FontDescriptor::from_name("romans.SHX", false, false).is_shx());
        let arial = FontDescriptor::from_name("Arial", false, true);
        assert_eq!(arial.name_and_flags(), ("Arial".into(), false, true));
    }

    #[test]
    fn bad_styles_are_found() {
        let ok = TextStyle::new(TextStyleId(1), "Notes");
        assert!(ok.is_valid());
        let long = "x".repeat(MAX_NAME_BYTES + 1);
        let bad = [
            TextStyle::new(TextStyleId(1), "  "),
            TextStyle::new(TextStyleId(1), long.clone()),
            TextStyle {
                width_factor: 0.0,
                ..ok.clone()
            },
            TextStyle {
                width_factor: f64::NAN,
                ..ok.clone()
            },
            TextStyle {
                oblique_radians: 2.0,
                ..ok.clone()
            },
            TextStyle {
                fixed_height: Some(-1.0),
                ..ok.clone()
            },
            TextStyle {
                font: FontDescriptor::Shx { file_name: long },
                ..ok
            },
        ];
        for style in bad {
            assert!(!style.is_valid(), "{style:?}");
        }
    }
}
