//! The point display settings in a DXF header: `$PDMODE` and `$PDSIZE`.

use od_core::{Document, PointStyle};

/// The header variables to write, as (name, group code, value).
pub(super) fn header(style: PointStyle) -> [(&'static str, i32, String); 2] {
    [
        ("$PDMODE", 70, style.mode.to_string()),
        ("$PDSIZE", 40, style.size.to_string()),
    ]
}

/// Set the point style from a header's groups (code, value). What is missing
/// or not allowed keeps the default.
pub(super) fn apply_header<'a>(
    groups: impl Iterator<Item = (i32, &'a str)>,
    document: &mut Document,
) {
    let mut style = PointStyle::default();
    let mut variable = "";
    for (code, value) in groups {
        if code == 9 {
            variable = value;
        } else if variable == "$PDMODE" && code == 70 {
            style.mode = value.parse().unwrap_or(style.mode);
        } else if variable == "$PDSIZE" && code == 40 {
            style.size = value.parse().unwrap_or(style.size);
        }
    }
    document.set_point_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_sets_the_mode_and_size_and_ignores_what_is_not_allowed() {
        let mut doc = Document::default();
        let groups = [(9, "$PDMODE"), (70, "35"), (9, "$PDSIZE"), (40, "-4")];
        apply_header(groups.into_iter(), &mut doc);
        assert_eq!(
            doc.point_style(),
            PointStyle {
                mode: 35,
                size: -4.0
            }
        );
        let bad = [(9, "$PDMODE"), (70, "9"), (9, "$PDSIZE"), (40, "abc")];
        let mut doc = Document::default();
        apply_header(bad.into_iter(), &mut doc);
        assert_eq!(doc.point_style(), PointStyle::default());
    }
}
