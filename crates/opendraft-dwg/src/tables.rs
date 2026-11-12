//! The symbol table records the importer needs: layers, linetypes and block
//! records (Open Design Specification §20.4.52, §20.4.54 and §20.4.58).
//!
//! Each begins with the common non-entity fields, which the caller has
//! already read, so these pick up at the record's own fields in the data and
//! string streams, and at the handles after the common ones.

use crate::common::{read_table_color, TableColor};
use crate::error::{DwgError, DwgErrorKind};
use crate::handles::DwgHandle;
use crate::limits::{check, DwgLimits};
use crate::object::ObjectStreams;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerData {
    pub name: String,
    pub frozen: bool,
    pub locked: bool,
    /// The "on" bit; see `read_layer` for how it is read.
    pub on: bool,
    /// The lineweight code (0 to 23 standard, 29 ByBlock, 30 ByLayer, 31 default).
    pub lineweight: u8,
    pub color: TableColor,
    pub linetype: Option<DwgHandle>,
}

fn next_handle(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<Option<DwgHandle>, DwgError> {
    let reference = s.handles.handle(limits.max_handle_bytes)?;
    reference
        .resolve(base)
        .map_err(|e| e.at(s.handles.position()))
}

/// LAYER. After the name come the xref index and the flags short: frozen (1),
/// off (2), frozen in new viewports (4), locked (8), plot (16) and the
/// lineweight (mask 0x03E0). The specification's separate 64-bit and
/// xref-dependent bits are not in the supplied drawings. The colour's index
/// is negative for a layer that is off, in drawings that follow that
/// convention, so the layer is off when either says so.
pub(crate) fn read_layer(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<LayerData, DwgError> {
    let name = s.strings.text(limits.max_string_code_units)?;
    s.data.bs()?; // xref index + 1
    let flags = s.data.bs()?;
    let color = read_table_color(&mut s.data, &mut s.strings, limits)?;
    next_handle(s, base, limits)?; // the external reference block
    next_handle(s, base, limits)?; // plot style
    next_handle(s, base, limits)?; // material
    let linetype = next_handle(s, base, limits)?;
    Ok(LayerData {
        name,
        frozen: flags & 1 != 0,
        locked: flags & 8 != 0,
        on: flags & 2 == 0 && (color.index as i16) >= 0,
        lineweight: ((flags & 0x03E0) >> 5) as u8,
        color,
        linetype,
    })
}

/// A linetype definition of the drawing's table: its name and, when it could be
/// read, its pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct DwgLinetypeDef {
    pub name: String,
    pub description: String,
    /// The dash lengths as DXF group 49 has them (positive draws, negative
    /// leaves a gap, zero is a dot); `None` when the pattern did not decode.
    pub pattern: Option<Vec<f64>>,
    /// Some dash carries text or a shape, which is not read.
    pub complex: bool,
}

/// LTYPE (§20.4.58): after the name come the xref index and, in the strings,
/// the description; then the pattern length, the alignment (always `A`), the
/// number of dashes and each dash's length and its complex-element fields.
/// A pattern that does not decode leaves the name alone, so the linetype
/// still carries across.
pub(crate) fn read_linetype(
    s: &mut ObjectStreams<'_>,
    limits: &DwgLimits,
) -> Result<DwgLinetypeDef, DwgError> {
    let name = s.strings.text(limits.max_string_code_units)?;
    let mut data = DwgLinetypeDef {
        name,
        description: String::new(),
        pattern: None,
        complex: false,
    };
    if let Ok((description, pattern, complex)) = read_pattern(s, limits) {
        data.description = description;
        data.pattern = Some(pattern);
        data.complex = complex;
    }
    Ok(data)
}

fn read_pattern(
    s: &mut ObjectStreams<'_>,
    limits: &DwgLimits,
) -> Result<(String, Vec<f64>, bool), DwgError> {
    s.data.bs()?; // xref index + 1
    let description = s.strings.text(limits.max_string_code_units)?;
    s.data.bd()?; // pattern length: the sum of the dashes
                  // The alignment is always `A`; anything else is not a pattern we know.
    if s.data.rc()? != b'A' {
        return Err(s.data.error(DwgErrorKind::InvalidObject));
    }
    let count = s.data.rc()?;
    let mut pattern = Vec::new();
    let mut complex = false;
    for _ in 0..count {
        let dash = s.data.bd()?;
        s.data.bs()?; // shape code
        s.data.rd()?; // x offset
        s.data.rd()?; // y offset
        s.data.bd()?; // scale
        s.data.bd()?; // rotation
        let flag = s.data.bs()?;
        complex |= flag != 0;
        pattern.push(dash);
    }
    Ok((description, pattern, complex))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockData {
    pub name: String,
    /// An anonymous block (`*D`, `*U`, ...): a dimension's or a dynamic
    /// block's picture, not a block the user named.
    pub anonymous: bool,
    pub is_xref: bool,
    /// The point an insert puts at its insertion point.
    pub base: [f64; 3],
    pub description: String,
    /// The entities the block owns, in drawing order.
    pub entities: Vec<DwgHandle>,
    pub layout: Option<DwgHandle>,
}

/// BLOCK_HEADER (a block table record). As for layers, the specification's
/// 64-bit and xref-dependent bits are not in the supplied drawings, and the
/// handles after the common ones start with a null pointer.
pub(crate) fn read_block_header(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<BlockData, DwgError> {
    let text_limit = limits.max_string_code_units;
    let name = s.strings.text(text_limit)?;
    s.data.bs()?; // xref index + 1
    let anonymous = s.data.bit()?;
    s.data.bit()?; // has attributes
    let is_xref = s.data.bit()?;
    let overlaid = s.data.bit()?;
    s.data.bit()?; // loaded
                   // An xref owns no entities, and the count is not stored for one.
    let owned = if is_xref || overlaid { 0 } else { s.data.bl()? };
    let base_point = s.data.bd3()?;
    s.strings.text(text_limit)?; // xref path
                                 // One non-zero byte per insert, ended by a zero byte.
    let mut inserts = 0u64;
    while s.data.rc()? != 0 {
        inserts += 1;
        check(
            s.data.stage(),
            "block inserts",
            inserts,
            limits.max_collection_items,
        )?;
    }
    let description = s.strings.text(text_limit)?;
    let preview = u64::from(s.data.bl()?);
    s.data.skip(
        preview
            .checked_mul(8)
            .ok_or_else(|| s.data.error(DwgErrorKind::InvalidObject))?,
    )?;
    s.data.bs()?; // insert units
    s.data.bit()?; // explodable
    s.data.rc()?; // block scaling

    check(
        s.data.stage(),
        "block entities",
        u64::from(owned),
        limits.max_collection_items,
    )?;
    if (u64::from(owned) + inserts) * 8 > s.handles.remaining() {
        return Err(s.handles.error(DwgErrorKind::Truncated));
    }
    next_handle(s, base, limits)?; // a null pointer
    next_handle(s, base, limits)?; // the BLOCK entity
    let mut entities = Vec::with_capacity(owned as usize);
    if !is_xref && !overlaid {
        for _ in 0..owned {
            if let Some(handle) = next_handle(s, base, limits)? {
                entities.push(handle);
            }
        }
    }
    next_handle(s, base, limits)?; // the ENDBLK entity
    for _ in 0..inserts {
        next_handle(s, base, limits)?;
    }
    let layout = next_handle(s, base, limits)?;
    Ok(BlockData {
        name,
        anonymous,
        is_xref,
        base: base_point,
        description,
        entities,
        layout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{BitWriter, Buffers};

    const BASE: DwgHandle = DwgHandle(0x10);

    fn layer_streams(flags: u16, index: u16, packed: u32) -> Buffers {
        let mut data = BitWriter::new();
        data.bs(0).bs(flags).bs(index).bl(packed).rc(0);
        let mut strings = BitWriter::new();
        strings.text("Walls");
        let mut handles = BitWriter::new();
        handles
            .handle(4, 0)
            .handle(4, 0)
            .handle(4, 0x0F)
            .handle(5, 0x14);
        Buffers::new(&data, &strings, &handles)
    }

    #[test]
    fn a_layer_with_its_flags_colour_and_linetype() {
        let b = layer_streams(1 | 8 | (7 << 5), 3, 0xC300_0003);
        let l = read_layer(&mut b.streams(), BASE, &DwgLimits::default()).unwrap();
        assert_eq!(l.name, "Walls");
        assert!(l.frozen && l.locked && l.on);
        assert_eq!(l.lineweight, 7);
        assert_eq!(l.color.aci(), Some(3));
        assert_eq!(l.color.true_color(), None);
        assert_eq!(l.linetype, Some(DwgHandle(0x14)));
    }

    #[test]
    fn a_layer_is_off_by_its_bit_or_its_negative_colour() {
        let b = layer_streams(2, 3, 0);
        assert!(
            !read_layer(&mut b.streams(), BASE, &DwgLimits::default())
                .unwrap()
                .on
        );
        let b = layer_streams(0, (-3i16) as u16, 0);
        assert!(
            !read_layer(&mut b.streams(), BASE, &DwgLimits::default())
                .unwrap()
                .on
        );
    }

    #[test]
    fn a_true_colour_layer() {
        let b = layer_streams(0, 7, 0xC2FF_8000);
        let l = read_layer(&mut b.streams(), BASE, &DwgLimits::default()).unwrap();
        assert_eq!(l.color.true_color(), Some(0xFF_8000));
    }

    #[test]
    fn a_block_record_lists_its_entities_in_order() {
        let mut data = BitWriter::new();
        data.bs(0)
            .bit(false)
            .bit(false)
            .bit(false)
            .bit(false)
            .bit(true);
        data.bl(3)
            .bd3([1.0, 2.0, 0.0])
            .rc(1)
            .rc(0)
            .bl(0)
            .bs(0)
            .bit(true)
            .rc(1);
        let mut strings = BitWriter::new();
        strings.text("*Model_Space").text("").text("Plan");
        let mut handles = BitWriter::new();
        handles.handle(5, 0); // null
        handles.handle(3, 0x20); // BLOCK
        handles.handle(4, 0x40).handle(4, 0x41).handle(4, 0x42); // entities
        handles.handle(3, 0x21); // ENDBLK
        handles.handle(4, 0x50); // the one insert
        handles.handle(4, 0x22); // layout
        let b = Buffers::new(&data, &strings, &handles);
        let block = read_block_header(&mut b.streams(), BASE, &DwgLimits::default()).unwrap();
        assert_eq!(block.name, "*Model_Space");
        assert!(!block.is_xref && !block.anonymous);
        assert_eq!(
            (block.base, block.description.as_str()),
            ([1.0, 2.0, 0.0], "Plan")
        );
        assert_eq!(
            block.entities,
            [DwgHandle(0x40), DwgHandle(0x41), DwgHandle(0x42)]
        );
        assert_eq!(block.layout, Some(DwgHandle(0x22)));
    }

    #[test]
    fn an_xref_block_stores_no_owned_count() {
        let mut data = BitWriter::new();
        data.bs(0)
            .bit(false)
            .bit(false)
            .bit(true)
            .bit(false)
            .bit(true);
        data.bd3([0.0; 3]).rc(0).bl(0).bs(0).bit(true).rc(1);
        let mut strings = BitWriter::new();
        strings.text("Sketch").text("..\\Sketch.dwg").text("");
        let mut handles = BitWriter::new();
        handles.handle(5, 0).handle(3, 0x20).handle(3, 0x21);
        handles.handle(4, 0x22);
        let b = Buffers::new(&data, &strings, &handles);
        let block = read_block_header(&mut b.streams(), BASE, &DwgLimits::default()).unwrap();
        assert!(block.is_xref && block.entities.is_empty());
        assert_eq!(block.layout, Some(DwgHandle(0x22)));
    }

    #[test]
    fn a_block_cannot_own_more_entities_than_handles_exist() {
        let mut data = BitWriter::new();
        data.bs(0)
            .bit(false)
            .bit(false)
            .bit(false)
            .bit(false)
            .bit(true);
        data.bl(100_000)
            .bd3([0.0; 3])
            .rc(0)
            .bl(0)
            .bs(0)
            .bit(true)
            .rc(1);
        let mut strings = BitWriter::new();
        strings.text("B").text("").text("");
        let b = Buffers::new(&data, &strings, BitWriter::new().handle(3, 1));
        let err = read_block_header(&mut b.streams(), BASE, &DwgLimits::default()).unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::Truncated);
    }

    /// The fields of one simple dash after its length.
    fn simple_dash(data: &mut BitWriter, length: f64) {
        data.bd(length).bs(0).rd(0.0).rd(0.0).bd(1.0).bd(0.0).bs(0);
    }

    #[test]
    fn a_linetype_with_its_pattern() {
        let mut data = BitWriter::new();
        data.bs(0).bd(0.75).rc(b'A').rc(2);
        simple_dash(&mut data, 0.5);
        simple_dash(&mut data, -0.25);
        let mut strings = BitWriter::new();
        strings.text("DASHED").text("Dashed __ __");
        let b = Buffers::new(&data, &strings, &BitWriter::new());
        let l = read_linetype(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert_eq!(
            (l.name.as_str(), l.description.as_str()),
            ("DASHED", "Dashed __ __")
        );
        assert_eq!(l.pattern, Some(vec![0.5, -0.25]));
        assert!(!l.complex);
    }

    #[test]
    fn a_dash_with_a_shape_or_text_makes_the_linetype_complex() {
        let mut data = BitWriter::new();
        data.bs(0).bd(0.5).rc(b'A').rc(1);
        data.bd(0.5).bs(1).rd(0.0).rd(0.0).bd(1.0).bd(0.0).bs(4);
        let mut strings = BitWriter::new();
        strings.text("FENCE").text("");
        let b = Buffers::new(&data, &strings, &BitWriter::new());
        let l = read_linetype(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert!(l.complex);
        assert_eq!(l.pattern, Some(vec![0.5]));
    }

    #[test]
    fn a_pattern_that_does_not_decode_leaves_the_name() {
        // Only the name is there; the rest of the record is missing.
        let mut strings = BitWriter::new();
        strings.text("DASHED");
        let b = Buffers::new(&BitWriter::new(), &strings, &BitWriter::new());
        let l = read_linetype(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert_eq!((l.name.as_str(), l.pattern), ("DASHED", None));
        // An alignment other than A is not a pattern we know either.
        let mut data = BitWriter::new();
        data.bs(0).bd(1.0).rc(b'B').rc(0);
        let mut strings = BitWriter::new();
        strings.text("ODD").text("");
        let b = Buffers::new(&data, &strings, &BitWriter::new());
        let l = read_linetype(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert_eq!(l.pattern, None);
    }
}
