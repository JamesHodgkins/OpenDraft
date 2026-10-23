//! The TABLES section of the R2000 writer: viewport, linetype, layer, text
//! style, view, UCS, application, dimension style and block-record tables.

use std::collections::BTreeSet;

use od_core::Document;

use super::{block_name, hex, true_color, weight_code, Handles, Ids};
use crate::dxf::write::{aci_for, dxf_name, Out};

pub(super) fn write_tables(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    linetypes: &BTreeSet<String>,
) {
    out.section("TABLES");

    let owner = table(out, h, "VPORT", 1);
    record(out, "VPORT", owner, "AcDbViewportTableRecord", h.next());
    out.pair(2, "*Active");
    out.pair(70, 0);
    for (code, x, y) in [
        (10, 0.0, 0.0),
        (11, 1.0, 1.0),
        (12, 0.0, 0.0),
        (13, 0.0, 0.0),
        (14, 0.5, 0.5),
        (15, 0.5, 0.5),
    ] {
        out.pair(code, x);
        out.pair(code + 10, y);
    }
    out.point3(16, 0.0, 0.0, 1.0);
    out.point3(17, 0.0, 0.0, 0.0);
    for (code, value) in [
        (40, "1000.0"),
        (41, "1.34"),
        (42, "50.0"),
        (43, "0.0"),
        (44, "0.0"),
        (50, "0.0"),
        (51, "0.0"),
        (71, "0"),
        (72, "1000"),
        (73, "1"),
        (74, "3"),
        (75, "0"),
        (76, "0"),
        (77, "0"),
        (78, "0"),
    ] {
        out.pair(code, value);
    }
    out.pair(0, "ENDTAB");

    let owner = table(out, h, "LTYPE", linetypes.len() + 3);
    for name in ["ByBlock", "ByLayer", "Continuous"] {
        let handle = if name == "ByLayer" {
            ids.bylayer_ltype
        } else {
            h.next()
        };
        record(out, "LTYPE", owner, "AcDbLinetypeTableRecord", handle);
        out.pair(2, name);
        out.pair(70, 0);
        out.pair(
            3,
            if name == "Continuous" {
                "Solid line"
            } else {
                ""
            },
        );
        out.pair(72, 65);
        out.pair(73, 0);
        out.pair(40, 0.0);
    }
    for name in linetypes {
        let (description, dashes) = crate::dxf::linetypes::definition(document, name);
        record(out, "LTYPE", owner, "AcDbLinetypeTableRecord", h.next());
        out.pair(2, name);
        out.pair(70, 0);
        out.pair(3, description);
        out.pair(72, 65);
        out.pair(73, dashes.len());
        out.pair(40, dashes.iter().map(|d| d.abs()).sum::<f64>());
        for dash in dashes {
            out.pair(49, dash);
            out.pair(74, 0);
        }
    }
    out.pair(0, "ENDTAB");

    let owner = table(out, h, "LAYER", document.layers().len());
    for layer in document.layers() {
        let handle = ids.layers[&layer.id];
        let (index, _) = aci_for(layer.color);
        record(out, "LAYER", owner, "AcDbLayerTableRecord", handle);
        out.pair(2, dxf_name(&layer.name));
        out.pair(70, i32::from(layer.frozen) + 4 * i32::from(layer.locked));
        out.pair(62, if layer.on { index } else { -index });
        if let Some(rgb) = true_color(layer.color) {
            out.pair(420, rgb);
        }
        out.pair(
            6,
            crate::dxf::linetypes::name_for(document, &layer.linetype),
        );
        out.pair(370, weight_code(layer.lineweight));
        out.pair(390, hex(ids.placeholder));
    }
    out.pair(0, "ENDTAB");

    super::text::styles(out, h, ids, document);

    table(out, h, "VIEW", 0);
    out.pair(0, "ENDTAB");
    table(out, h, "UCS", 0);
    out.pair(0, "ENDTAB");

    let owner = table(out, h, "APPID", 1);
    record(out, "APPID", owner, "AcDbRegAppTableRecord", h.next());
    out.pair(2, "ACAD");
    out.pair(70, 0);
    out.pair(0, "ENDTAB");

    let owner = table(out, h, "DIMSTYLE", document.dim_styles().len());
    out.pair(100, "AcDbDimStyleTable");
    for (style, handle) in document.dim_styles().iter().zip(&ids.dim_styles) {
        record(out, "DIMSTYLE", owner, "AcDbDimStyleTableRecord", *handle);
        out.pair(2, dxf_name(&style.name));
        let text_style = document
            .text_styles()
            .iter()
            .position(|t| t.id == style.text_style)
            .and_then(|i| ids.text_styles.get(i).copied());
        for (code, value) in super::dimension::style_pairs(style, text_style) {
            out.pair(code, value);
        }
    }
    out.pair(0, "ENDTAB");

    let owner = table(
        out,
        h,
        "BLOCK_RECORD",
        document.layouts().len() + 1 + ids.dim_blocks.len(),
    );
    let block_record = |out: &mut Out, name: &str, handle: u32, layout: u32| {
        record(out, "BLOCK_RECORD", owner, "AcDbBlockTableRecord", handle);
        out.pair(2, name);
        out.pair(340, hex(layout));
    };
    block_record(out, "*Model_Space", ids.model_record, ids.model_layout);
    for (i, _) in document.layouts().iter().enumerate() {
        block_record(out, &block_name(i), ids.records[i], ids.layouts[i]);
    }
    for (name, record_handle) in ids.dim_blocks.values() {
        block_record(out, name, *record_handle, 0);
    }
    out.pair(0, "ENDTAB");
    out.end_section();
}

/// The start of a table; returns its handle (the owner of its records).
pub(super) fn table(out: &mut Out, h: &mut Handles, name: &str, count: usize) -> u32 {
    let handle = h.next();
    out.pair(0, "TABLE");
    out.pair(2, name);
    out.pair(5, hex(handle));
    out.pair(330, 0);
    out.pair(100, "AcDbSymbolTable");
    out.pair(70, count);
    handle
}

/// The common start of a table record.
pub(super) fn record(out: &mut Out, kind: &str, owner: u32, subclass: &str, handle: u32) {
    out.pair(0, kind);
    out.pair(if kind == "DIMSTYLE" { 105 } else { 5 }, hex(handle));
    out.pair(330, hex(owner));
    out.pair(100, "AcDbSymbolTableRecord");
    out.pair(100, subclass);
}

/// DIMSTYLE "Standard" with AutoCAD's defaults (group code, value).
pub(super) const DIMSTYLE: &[(i32, &str)] = &[
    (70, "0"),
    (3, ""),
    (4, ""),
    (40, "1.0"),
    (41, "2.5"),
    (42, "0.625"),
    (43, "3.75"),
    (44, "1.25"),
    (45, "0.0"),
    (46, "0.0"),
    (47, "0.0"),
    (48, "0.0"),
    (140, "2.5"),
    (141, "2.5"),
    (142, "0.0"),
    (143, "0.03937007874"),
    (144, "1.0"),
    (145, "0.0"),
    (146, "1.0"),
    (147, "0.625"),
    (148, "0.0"),
    (71, "0"),
    (72, "0"),
    (73, "0"),
    (74, "0"),
    (75, "0"),
    (76, "0"),
    (77, "1"),
    (78, "8"),
    (79, "3"),
    (170, "0"),
    (171, "3"),
    (172, "1"),
    (173, "0"),
    (174, "0"),
    (175, "0"),
    (176, "0"),
    (177, "0"),
    (178, "0"),
    (179, "2"),
    (271, "2"),
    (272, "2"),
    (273, "2"),
    (274, "3"),
    (275, "0"),
    (276, "0"),
    (277, "2"),
    (278, "44"),
    (279, "0"),
    (280, "0"),
    (281, "0"),
    (282, "0"),
    (283, "0"),
    (284, "8"),
    (285, "0"),
    (286, "0"),
    (288, "0"),
    (289, "3"),
    (371, "-2"),
    (372, "-2"),
];
