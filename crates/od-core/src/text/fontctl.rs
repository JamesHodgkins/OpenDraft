//! The font control, `\fArial|b1|i0|c0|p34;`: a family, then fields. OD
//! Roman adds `|w0.18`, its pen width as a fraction of the text height; a
//! reader that does not know the field ignores it.

use super::stroke_outline::{MAX_WEIGHT, MIN_WEIGHT};

/// The style flags of a font control. A font control replaces the font, so a
/// flag it leaves out is off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontFlags {
    pub bold: bool,
    pub italic: bool,
    /// The pen width of a stroke font in thousandths of the text height;
    /// `None` leaves it to the style.
    pub weight: Option<u16>,
}

/// The parts of a font control's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontParts<'a> {
    pub family: &'a str,
    pub flags: FontFlags,
    /// The character set (`c`) and pitch and family (`p`) fields, as written.
    pub charset: Option<&'a str>,
    pub pitch: Option<&'a str>,
}

/// Split the text of a font control, with or without its `\f` and `;`.
pub fn font_parts(control: &str) -> FontParts<'_> {
    let body = control
        .strip_prefix("\\f")
        .or_else(|| control.strip_prefix("\\F"))
        .unwrap_or(control);
    let body = body.strip_suffix(';').unwrap_or(body);
    let mut fields = body.split('|');
    let mut parts = FontParts {
        family: fields.next().unwrap_or(""),
        flags: FontFlags::default(),
        charset: None,
        pitch: None,
    };
    for field in fields {
        let split = field.chars().next().map_or(0, char::len_utf8);
        let (key, value) = field.split_at(split);
        match key {
            "b" => parts.flags.bold = value == "1",
            "i" => parts.flags.italic = value == "1",
            "c" => parts.charset = Some(value),
            "p" => parts.pitch = Some(value),
            "w" => parts.flags.weight = parse_weight(value),
            _ => {}
        }
    }
    parts
}

/// `0.18` as thousandths, if it is a usable weight.
fn parse_weight(value: &str) -> Option<u16> {
    let v: f64 = value.parse().ok().filter(|v: &f64| v.is_finite())?;
    Some((v.clamp(MIN_WEIGHT, MAX_WEIGHT) * 1000.0).round() as u16)
}

/// A font control for `family` with the given flags, keeping the character
/// set and pitch fields of the one it replaces.
pub fn font_control(family: &str, flags: FontFlags, keep: Option<&FontParts<'_>>) -> String {
    let mut out = format!(
        "\\f{family}|b{}|i{}",
        u8::from(flags.bold),
        u8::from(flags.italic)
    );
    if let Some(w) = flags.weight {
        out.push_str(&format!("|w{}", f64::from(w) / 1000.0));
    }
    if let Some(c) = keep.and_then(|k| k.charset) {
        out.push_str(&format!("|c{c}"));
    }
    if let Some(p) = keep.and_then(|k| k.pitch) {
        out.push_str(&format!("|p{p}"));
    }
    out.push(';');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_control_is_split_into_its_parts() {
        let p = font_parts("\\fArial|b1|i0|c0|p34;");
        assert_eq!(p.family, "Arial");
        assert_eq!(
            p.flags,
            FontFlags {
                bold: true,
                italic: false,
                weight: None
            }
        );
        assert_eq!((p.charset, p.pitch), (Some("0"), Some("34")));
        let bare = font_parts("\\fDejaVu Sans;");
        assert_eq!(bare.family, "DejaVu Sans");
        assert_eq!(bare.flags, FontFlags::default());
        assert_eq!((bare.charset, bare.pitch), (None, None));
        assert!(font_parts("Times New Roman|i1").flags.italic);
        assert_eq!(font_parts("\\Fromans.shx;").family, "romans.shx");
        assert_eq!(font_parts("").family, "");
        // Odd fields are ignored, and a multi-byte one does not panic.
        assert!(font_parts("A|é1|b1|").flags.bold);
    }

    #[test]
    fn a_made_font_control_reads_back_and_keeps_charset_and_pitch() {
        let old = font_parts("\\fArial|b0|i0|c204|p34;");
        let made = font_control(
            "Arial",
            FontFlags {
                bold: true,
                italic: true,
                weight: None,
            },
            Some(&old),
        );
        assert_eq!(made, "\\fArial|b1|i1|c204|p34;");
        let back = font_parts(&made);
        assert_eq!(
            back.flags,
            FontFlags {
                bold: true,
                italic: true,
                weight: None
            }
        );
        assert_eq!(
            font_control("DejaVu Sans", FontFlags::default(), None),
            "\\fDejaVu Sans|b0|i0;"
        );
    }

    #[test]
    fn a_weight_field_is_read_written_and_kept_in_range() {
        let p = font_parts("\\fOD Roman|b0|i0|w0.18;");
        assert_eq!(p.flags.weight, Some(180));
        let flags = FontFlags {
            weight: Some(125),
            ..FontFlags::default()
        };
        assert_eq!(
            font_control("OD Roman", flags, None),
            "\\fOD Roman|b0|i0|w0.125;"
        );
        assert_eq!(font_parts("A|w9").flags.weight, Some(400));
        assert_eq!(font_parts("A|w0").flags.weight, Some(10));
        assert_eq!(font_parts("A|wx").flags.weight, None);
        assert_eq!(font_parts("A|wNaN").flags.weight, None);
        assert_eq!(font_parts("A|b1").flags.weight, None);
    }
}
