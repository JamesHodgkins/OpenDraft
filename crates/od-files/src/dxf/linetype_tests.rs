//! Linetype definitions through DXF, in both directions.

use od_core::{Attr, Attributes, Document, Entity, LayerId, Point, StrokeElement};

use super::read::read;
use super::{write, write2000};

fn dxf(pairs: &[(i32, &str)]) -> Vec<u8> {
    let mut text = String::new();
    for (code, value) in pairs {
        text.push_str(&format!("{code:>3}\r\n{value}\r\n"));
    }
    text.into_bytes()
}

/// A file with an LTYPE table, a layer on `layer_type` and one line on `line_type`.
fn file(ltypes: &[(i32, &str)], layer_type: &str, line_type: &str) -> Vec<u8> {
    let mut pairs = vec![(0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LTYPE")];
    pairs.extend_from_slice(ltypes);
    pairs.extend([(0, "ENDTAB"), (0, "TABLE"), (2, "LAYER")]);
    pairs.extend([
        (0, "LAYER"),
        (2, "L"),
        (70, "0"),
        (62, "7"),
        (6, layer_type),
    ]);
    pairs.extend([
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
    ]);
    pairs.extend([(0, "LINE"), (8, "L"), (6, line_type)]);
    pairs.extend([(10, "0"), (20, "0"), (11, "9"), (21, "0")]);
    pairs.extend([(0, "ENDSEC"), (0, "EOF")]);
    dxf(&pairs)
}

/// An LTYPE record with simple dashes.
fn ltype<'a>(name: &'a str, description: &'a str, dashes: &[&'a str]) -> Vec<(i32, &'a str)> {
    let mut pairs = vec![(0, "LTYPE"), (2, name), (70, "0"), (3, description)];
    pairs.extend([(72, "65"), (40, "1")]);
    pairs.extend(dashes.iter().map(|d| (49, *d)));
    pairs
}

#[test]
fn a_definition_in_the_file_is_the_one_drawn() {
    let mut table = ltype("DASHED", "Dashed", &["0.5", "-0.25"]);
    table.extend(ltype("MINE", "My line", &["3", "-1", "0", "-1"]));
    let loaded = read(&file(&table, "DASHED", "MINE")).unwrap();
    let doc = loaded.document;
    // The file's imperial DASHED replaced our stock one, keeping its id.
    let dashed = doc.linetypes().named("DASHED").unwrap();
    assert_eq!(dashed.id.0, 4);
    assert_eq!(
        dashed.elements,
        vec![StrokeElement::Dash(0.5), StrokeElement::Gap(0.25)]
    );
    let mine = doc.linetypes().named("mine").unwrap();
    assert_eq!(mine.description, "My line");
    assert_eq!(mine.elements.len(), 4);
    assert_eq!(mine.elements[2], StrokeElement::Dot);
    let (elements, _) = doc.dash_pattern(&doc.objects()[0]).unwrap();
    assert_eq!(elements, mine.elements.as_slice());
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn the_special_records_in_a_file_change_nothing() {
    let mut table = ltype("BYBLOCK", "", &[]);
    table.extend(ltype("ByLayer", "", &[]));
    table.extend(ltype("Continuous", "Solid line", &[]));
    let doc = read(&file(&table, "Continuous", "ByLayer"))
        .unwrap()
        .document;
    assert!(doc.linetypes().is_stock());
}

#[test]
fn a_name_the_file_never_defines_gets_the_stock_pattern_or_none_and_is_reported() {
    let loaded = read(&file(&[], "HIDDEN2", "NOPE")).unwrap();
    let doc = &loaded.document;
    let hidden = doc.linetypes().named("HIDDEN2").unwrap();
    assert_eq!(hidden.elements.len(), 2);
    assert!(doc.linetypes().named("NOPE").is_none());
    assert!(doc.dash_pattern(&doc.objects()[0]).is_none());
    let text = loaded.warnings.join("\n");
    assert!(text.contains("standard pattern: HIDDEN2"), "{text}");
    assert!(text.contains("drawn solid: NOPE"), "{text}");
}

#[test]
fn text_and_shapes_in_a_definition_are_reported_and_the_dashes_kept() {
    let table = vec![
        (0, "LTYPE"),
        (2, "GAS_LINE"),
        (70, "0"),
        (3, "Gas line ----GAS----"),
        (72, "65"),
        (73, "3"),
        (40, "1.2"),
        (49, "0.5"),
        (74, "0"),
        (49, "-0.2"),
        (74, "2"),
        (75, "0"),
        (49, "-0.5"),
        (74, "0"),
    ];
    let loaded = read(&file(&table, "GAS_LINE", "GAS_LINE")).unwrap();
    let gas = loaded.document.linetypes().named("GAS_LINE").unwrap();
    assert_eq!(gas.elements.len(), 3);
    let text = loaded.warnings.join("\n");
    assert!(
        text.contains("text or shapes") && text.contains("GAS_LINE"),
        "{text}"
    );
}

#[test]
fn a_bad_dash_length_skips_that_record_only() {
    let mut table = ltype("BAD", "", &["x", "-1"]);
    table.extend(ltype("GOOD", "", &["2", "-1"]));
    let doc = read(&file(&table, "GOOD", "GOOD")).unwrap().document;
    assert!(doc.linetypes().named("BAD").is_none());
    assert!(doc.linetypes().named("GOOD").is_some());
}

fn drawing() -> Document {
    let mut doc = Document::default();
    let mut table = doc.linetypes().clone();
    table
        .add(
            "MyLine",
            "mine",
            vec![
                StrokeElement::Dash(3.5),
                StrokeElement::Gap(1.25),
                StrokeElement::Dot,
                StrokeElement::Gap(1.25),
            ],
        )
        .unwrap();
    table.load_stock("ACAD_ISO04W100").unwrap();
    // Our DASHED, with other numbers than the stock ones.
    let dashed = table.named("DASHED").unwrap().clone();
    table
        .replace(od_core::Linetype {
            elements: vec![StrokeElement::Dash(2.0), StrokeElement::Gap(1.0)],
            ..dashed
        })
        .unwrap();
    doc.set_linetypes(table);
    let on = |name: &str| Attributes {
        linetype: Attr::Set(name.to_owned()),
        ..Attributes::on_layer(LayerId::DEFAULT)
    };
    let line = || Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(50.0, 0.0),
    };
    doc.add_with(line(), on("MyLine"));
    doc.add_with(line(), on("DOT"));
    doc
}

#[test]
fn the_table_survives_r12_and_r2000() {
    let original = drawing();
    for bytes in [
        write::write(&original).unwrap().bytes,
        write2000::write(&original).unwrap().bytes,
    ] {
        let back = read(&bytes).unwrap().document;
        for name in ["MyLine", "ACAD_ISO04W100", "DASHED"] {
            let was = original.linetypes().named(name).unwrap();
            let now = back.linetypes().named(name).unwrap();
            assert_eq!(now.elements, was.elements, "{name}");
            assert_eq!(now.description, was.description, "{name}");
        }
        let dashed = back.linetypes().named("DASHED").unwrap();
        assert_eq!(
            dashed.elements[0],
            StrokeElement::Dash(2.0),
            "not stock 12.7"
        );
        // The objects still draw dashed: the custom one and DOT.
        assert!(back
            .objects()
            .iter()
            .all(|o| back.dash_pattern(o).is_some()));
    }
}
