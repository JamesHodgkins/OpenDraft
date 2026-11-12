//! The drawing's own description, as AutoCAD's Drawing Properties (`DWGPROPS`)
//! Summary tab keeps it: who made it and what it is. Data fields can show
//! these (`Title`, `Author`...).

/// Longest value of any one property, in bytes.
pub const MAX_PROPERTY_BYTES: usize = 4096;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrawingProperties {
    pub title: String,
    pub subject: String,
    pub author: String,
    pub keywords: String,
    pub comments: String,
    /// The base address relative links in the drawing start from.
    pub hyperlink_base: String,
}

impl DrawingProperties {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn values(&self) -> [&String; 6] {
        [
            &self.title,
            &self.subject,
            &self.author,
            &self.keywords,
            &self.comments,
            &self.hyperlink_base,
        ]
    }

    /// Whether every value is within the limits: not too long, no NUL.
    pub fn validate(&self) -> Result<(), String> {
        if self
            .values()
            .iter()
            .any(|v| v.len() > MAX_PROPERTY_BYTES || v.contains('\0'))
        {
            return Err("a drawing property is too long or holds NUL".into());
        }
        Ok(())
    }

    /// The property called `name` (`Title`, `Author`...), in any case.
    pub fn named(&self, name: &str) -> Option<&str> {
        let value = match name.to_ascii_lowercase().as_str() {
            "title" => &self.title,
            "subject" => &self.subject,
            "author" => &self.author,
            "keywords" => &self.keywords,
            "comments" => &self.comments,
            "hyperlinkbase" => &self.hyperlink_base,
            _ => return None,
        };
        Some(value)
    }

    /// The same properties with every value cut to the limits and cleared of
    /// NUL, and line breaks in the single-line ones turned into spaces.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let clean = |s: &str, multiline: bool| -> String {
            let s: String = s
                .chars()
                .filter(|c| *c != '\0')
                .map(|c| if !multiline && c.is_control() { ' ' } else { c })
                .collect();
            let mut end = s.len().min(MAX_PROPERTY_BYTES);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s[..end].to_owned()
        };
        Self {
            title: clean(&self.title, false),
            subject: clean(&self.subject, false),
            author: clean(&self.author, false),
            keywords: clean(&self.keywords, false),
            comments: clean(&self.comments, true),
            hyperlink_base: clean(&self.hyperlink_base, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_set_is_empty_and_valid() {
        let p = DrawingProperties::default();
        assert!(p.is_empty() && p.validate().is_ok());
    }

    #[test]
    fn properties_are_found_by_name_in_any_case() {
        let p = DrawingProperties {
            author: "Ada".into(),
            hyperlink_base: "http://x".into(),
            ..DrawingProperties::default()
        };
        assert_eq!(p.named("AUTHOR"), Some("Ada"));
        assert_eq!(p.named("HyperlinkBase"), Some("http://x"));
        assert_eq!(p.named("Nope"), None);
    }

    #[test]
    fn too_long_or_nul_is_invalid_and_sanitizing_repairs_it() {
        let long = DrawingProperties {
            title: "é".repeat(MAX_PROPERTY_BYTES),
            comments: "a\0b\nc".into(),
            author: "x\ny".into(),
            ..DrawingProperties::default()
        };
        assert!(long.validate().is_err());
        let fixed = long.sanitized();
        assert!(fixed.validate().is_ok());
        assert!(fixed.title.len() <= MAX_PROPERTY_BYTES);
        assert_eq!(fixed.comments, "ab\nc", "comments keep their line breaks");
        assert_eq!(fixed.author, "x y", "a single line has none");
    }
}
