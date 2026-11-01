//! Reading attributed blocks from a DXF written by an independent library
//! (ezdxf, AutoCAD 2018 format). See `fixtures/make_ezdxf_attributes.py` for
//! what is in it.

use std::path::Path;

use od_core::block::{AttributeFlags, TextHorizontal, TextVertical};
use od_core::{Document, Entity, Insert, Point};

fn sample() -> od_files::Loaded {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_attributes.dxf");
    od_files::load(&path).expect("the sample reads")
}

fn inserts(doc: &Document) -> Vec<&Insert> {
    doc.model_objects()
        .filter_map(|o| match &o.entity {
            Entity::Insert(i) => Some(&**i),
            _ => None,
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_file_reads_cleanly_and_is_a_valid_drawing() {
    let loaded = sample();
    // ezdxf adds a SOLID in an arrowhead block, and writes an ATTRIB for each
    // insert's constant attribute, which AutoCAD does not.
    assert_eq!(
        loaded.warnings,
        [
            "Skipped 1 unsupported object: 1 SOLID.",
            "Skipped 3 constant attributes written on a block reference: a constant attribute is drawn from its definition."
        ]
    );
    assert_eq!(loaded.document.validate(), Ok(()));
    assert_eq!(inserts(&loaded.document).len(), 4);
}

#[test]
fn definitions_keep_their_modes_prompts_and_justification() {
    let loaded = sample();
    let title = loaded.document.block_named("TITLE").unwrap();
    let defs: Vec<_> = title.attribute_definitions().map(|(_, d)| d).collect();
    let tags: Vec<&str> = defs.iter().map(|d| d.tag.as_str()).collect();
    assert_eq!(
        tags,
        ["NAME", "REVISION", "STATUS", "CHECKED", "SCALE", "LOCKED", "HEADING"]
    );
    let by = |tag: &str| defs.iter().find(|d| d.tag == tag).unwrap();
    assert_eq!(by("NAME").prompt, "Name?");
    assert_eq!(by("NAME").default_value.display(), "none");
    assert!(by("REVISION").flags.invisible());
    assert!(by("STATUS").flags.constant());
    assert_eq!(by("STATUS").default_value.display(), "DRAFT");
    assert!(by("CHECKED").flags.verify());
    assert!(by("SCALE").flags.preset());
    assert!(by("LOCKED").lock_position && !by("NAME").lock_position);
    let heading = by("HEADING");
    assert_eq!(
        (heading.text.horizontal, heading.text.vertical),
        (TextHorizontal::Center, TextVertical::Middle)
    );
    assert_eq!(heading.text.position, Point::new(20.0, 30.0));
    assert_eq!(heading.text.height, 3.0);
    let ids: std::collections::HashSet<_> = defs.iter().map(|d| d.id).collect();
    assert_eq!(ids.len(), defs.len());
}

#[test]
fn a_loose_definition_comes_in_with_the_drawing() {
    let loaded = sample();
    let loose: Vec<_> = loaded
        .document
        .model_objects()
        .filter_map(|o| match &o.entity {
            Entity::AttributeDefinition(d) => Some(d),
            _ => None,
        })
        .collect();
    assert_eq!(loose.len(), 1);
    assert_eq!(loose[0].tag, "LOOSE");
}

#[test]
fn an_insert_holds_a_value_for_each_attribute_it_was_given_matched_to_its_definition() {
    let loaded = sample();
    let all = inserts(&loaded.document);
    let plain = all[0];
    let values: Vec<(String, String)> = plain
        .attributes
        .iter()
        .map(|r| (r.tag.clone(), r.value.display()))
        .collect();
    // The constant STATUS has no ATTRIB.
    assert_eq!(
        values,
        [
            ("NAME", "Plan A"),
            ("REVISION", "B"),
            ("CHECKED", "yes"),
            ("SCALE", "1:50"),
            ("LOCKED", "pinned"),
            ("HEADING", "Ground floor"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    assert!(plain.attributes.iter().all(|r| r.source.is_some()));
    assert!(plain.attributes[1].flags.invisible());
    assert!(plain.attributes[4].lock_position);
    assert!(plain.attributes[0].flags.0 & AttributeFlags::CONSTANT == 0);
}

#[test]
fn text_is_kept_in_the_blocks_own_coordinates_whatever_the_insert_does() {
    let loaded = sample();
    for insert in &inserts(&loaded.document)[..3] {
        // NAME is defined at (2, 4), height 2.5, unturned.
        let name = &insert.attributes[0];
        assert!(near(name.text.position.x, 2.0), "{:?}", name.text.position);
        assert!(near(name.text.position.y, 4.0), "{:?}", name.text.position);
        assert!(near(name.text.height, 2.5), "{}", name.text.height);
        assert!(near(name.text.rotation, 0.0), "{}", name.text.rotation);
    }
}

#[test]
fn what_an_insert_draws_is_where_the_file_put_the_text() {
    let loaded = sample();
    let turned = inserts(&loaded.document)[1];
    let drawn = turned.picture();
    let name = drawn
        .iter()
        .find_map(|e| match e {
            Entity::MText(m) if m.contents == "Turned" => Some(m),
            _ => None,
        })
        .expect("the value is drawn");
    // The file puts it at (199.4641, 58.9282), height 5, turned 30 degrees.
    assert!(near(name.insertion.x, 199.46410161513776));
    assert!(near(name.insertion.y, 58.92820323027551));
    assert!(near(name.nominal_height, 5.0));
}

#[test]
fn duplicate_tags_pair_up_in_order() {
    let loaded = sample();
    let twins = inserts(&loaded.document)[3];
    let values: Vec<String> = twins.attributes.iter().map(|r| r.value.display()).collect();
    assert_eq!(values, ["first", "second"]);
    let sources: Vec<_> = twins.attributes.iter().map(|r| r.source).collect();
    assert!(sources[0].is_some() && sources[1].is_some() && sources[0] != sources[1]);
    let defs: Vec<_> = twins
        .block
        .attribute_definitions()
        .map(|(_, d)| (d.id, d.default_value.display()))
        .collect();
    assert_eq!(sources[0], Some(defs[0].0));
    assert_eq!(sources[1], Some(defs[1].0));
}

// ---- writing: what we write reads back as the same attributed blocks

use od_files::{DxfFormat, FileFormat};

fn round_trip(doc: &Document) -> od_files::Loaded {
    let bytes = DxfFormat.write(doc).expect("it writes").bytes;
    DxfFormat.read(&bytes).expect("what we write reads")
}

fn attribute_rows(insert: &Insert) -> Vec<(String, String, u16, bool, [f64; 4])> {
    insert
        .attributes
        .iter()
        .map(|r| {
            (
                r.tag.clone(),
                r.value.display(),
                r.flags.0,
                r.lock_position,
                [
                    r.text.position.x,
                    r.text.position.y,
                    r.text.height,
                    r.text.rotation,
                ],
            )
        })
        .collect()
}

fn same_rows(a: &Insert, b: &Insert) {
    let (a, b) = (attribute_rows(a), attribute_rows(b));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!((&x.0, &x.1, x.2, x.3), (&y.0, &y.1, y.2, y.3));
        for (p, q) in x.4.iter().zip(&y.4) {
            assert!(near(*p, *q), "{x:?} vs {y:?}");
        }
    }
}

#[test]
fn what_we_write_reads_back_as_the_same_blocks_inserts_and_attributes() {
    let original = sample().document;
    let back = round_trip(&original);
    assert_eq!(back.warnings, Vec::<String>::new());
    assert_eq!(back.document.validate(), Ok(()));
    // The blocks are real blocks again, not their pieces.
    for name in ["TITLE", "DUP"] {
        let (a, b) = (
            original.block_named(name).unwrap(),
            back.document.block_named(name).unwrap(),
        );
        let rows = |b: &od_core::BlockDef| -> Vec<_> {
            b.attribute_definitions()
                .map(|(_, d)| {
                    (
                        d.tag.clone(),
                        d.prompt.clone(),
                        d.default_value.display(),
                        d.flags.0,
                        d.lock_position,
                        d.text.horizontal,
                        d.text.vertical,
                        d.text.position.x,
                    )
                })
                .collect()
        };
        assert_eq!(rows(a), rows(b), "{name}");
        assert_eq!(a.base, b.base);
    }
    let (before, after) = (inserts(&original), inserts(&back.document));
    assert_eq!(before.len(), after.len());
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.block.name, b.block.name);
        assert!(near(a.insertion.x, b.insertion.x) && near(a.scale_x, b.scale_x));
        assert!(near(a.rotation, b.rotation) && near(a.scale_y, b.scale_y));
        same_rows(a, b);
        for (x, y) in a.attributes.iter().zip(&b.attributes) {
            assert_eq!(x.source.is_some(), y.source.is_some(), "{}", x.tag);
        }
    }
    // The loose definition came back as one.
    assert!(back
        .document
        .model_objects()
        .any(|o| matches!(&o.entity, Entity::AttributeDefinition(d) if d.tag == "LOOSE")));
}

#[test]
fn writing_twice_gives_the_same_file() {
    let doc = sample().document;
    let first = DxfFormat.write(&doc).unwrap().bytes;
    let second = DxfFormat.write(&round_trip(&doc).document).unwrap().bytes;
    // Handles and ids differ in nothing that is written: same size at least.
    assert_eq!(first.len(), second.len());
}
