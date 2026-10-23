//! What a DXF header sets on the drawing when it is read.

use od_core::Document;

/// Apply the header's groups (code, value) to `document`: its units, linetype
/// scale and point style.
pub(super) fn apply<'a>(
    groups: impl Iterator<Item = (i32, &'a str)> + Clone,
    document: &mut Document,
) {
    document.set_units(super::units::read_header(groups.clone()));
    super::linetypes::apply_header(groups.clone(), document);
    super::point_style::apply_header(groups, document);
}
