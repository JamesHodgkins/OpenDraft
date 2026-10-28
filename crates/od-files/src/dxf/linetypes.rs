//! The linetype scale in DXF: the drawing's `$LTSCALE` and an entity's group
//! 48 and, for polylines, bit 128 of group 70 (Linetype Generation).

use od_core::linetype::stock;
use od_core::{Document, LinetypeId};

use super::write::linetype_for;

/// The name a linetype is written under: that of its record in the table, the
/// way DXF spells names.
pub(super) fn name_for(document: &Document, id: LinetypeId) -> String {
    let record = document.linetypes().get(id);
    linetype_for(record.map_or("CONTINUOUS", |r| r.name.as_str()))
}

/// The table's own records as written, which every writer defines whether or
/// not anything uses them (the three special ones it writes itself).
pub(super) fn table_names(document: &Document) -> impl Iterator<Item = String> + '_ {
    document
        .linetypes()
        .records()
        .iter()
        .filter(|r| !r.id.is_special())
        .map(|r| linetype_for(&r.name))
}

/// The description and dash lengths (positive draws, negative leaves a gap,
/// zero is a dot) written for the linetype `written`: the drawing's record,
/// else the stock one of that name, else a plain solid line.
pub(super) fn definition(document: &Document, written: &str) -> (String, Vec<f64>) {
    let record = document
        .linetypes()
        .records()
        .iter()
        .find(|r| r.id != LinetypeId::CONTINUOUS && linetype_for(&r.name) == written);
    if let Some(r) = record {
        return (
            r.description.clone(),
            r.elements.iter().map(|e| e.to_dxf()).collect(),
        );
    }
    match stock::find(written) {
        Some(s) => (s.description.to_owned(), s.pattern.to_vec()),
        None => ("Solid line".to_owned(), Vec::new()),
    }
}

/// Group 70 bit: the pattern runs continuously through a polyline's vertices.
pub(super) const CONTINUOUS_GENERATION: i64 = 128;

/// An entity's linetype scale (group 48, default 1) and Linetype Generation
/// (polylines only).
pub(super) fn read_entity(name: &str, scale: Option<f64>, flags: Option<i64>) -> (f64, bool) {
    let polyline = matches!(name, "LWPOLYLINE" | "POLYLINE");
    (
        scale.filter(|s| *s > 0.0).unwrap_or(1.0),
        polyline && flags.unwrap_or(0) & CONTINUOUS_GENERATION != 0,
    )
}

/// Set `$LTSCALE` from a header's groups (code, value) on `document`.
pub(super) fn apply_header<'a>(
    groups: impl Iterator<Item = (i32, &'a str)>,
    document: &mut od_core::Document,
) {
    if let Some(scale) = read_header(groups) {
        document.set_linetype_scale(scale);
    }
}

/// `$LTSCALE` from a header's groups: above zero, else `None`.
fn read_header<'a>(groups: impl Iterator<Item = (i32, &'a str)>) -> Option<f64> {
    let mut variable = "";
    for (code, value) in groups {
        if code == 9 {
            variable = value;
        } else if variable == "$LTSCALE" && code == 40 {
            return value
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v > 0.0);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_scale_is_read_when_it_is_a_usable_number() {
        let groups = [(9, "$LUNITS"), (70, "2"), (9, "$LTSCALE"), (40, "2.5")];
        assert_eq!(read_header(groups.into_iter()), Some(2.5));
        for bad in ["0", "-1", "x", "nan"] {
            let groups = [(9, "$LTSCALE"), (40, bad)];
            assert_eq!(read_header(groups.into_iter()), None, "{bad}");
        }
        assert_eq!(read_header(std::iter::empty()), None);
    }
}
