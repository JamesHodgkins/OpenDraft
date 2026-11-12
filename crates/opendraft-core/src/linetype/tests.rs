use super::*;

/// `acad.lin` (imperial) values, entered separately from `stock.rs`: the
/// metric families are these times 25.4.
const IMPERIAL: &[(&str, &[f64])] = &[
    ("BORDER", &[0.5, -0.25, 0.5, -0.25, 0.0, -0.25]),
    ("BORDER2", &[0.25, -0.125, 0.25, -0.125, 0.0, -0.125]),
    ("BORDERX2", &[1.0, -0.5, 1.0, -0.5, 0.0, -0.5]),
    ("CENTER", &[1.25, -0.25, 0.25, -0.25]),
    ("CENTER2", &[0.75, -0.125, 0.125, -0.125]),
    ("CENTERX2", &[2.5, -0.5, 0.5, -0.5]),
    ("DASHDOT", &[0.5, -0.25, 0.0, -0.25]),
    ("DASHDOT2", &[0.25, -0.125, 0.0, -0.125]),
    ("DASHDOTX2", &[1.0, -0.5, 0.0, -0.5]),
    ("DASHED", &[0.5, -0.25]),
    ("DASHED2", &[0.25, -0.125]),
    ("DASHEDX2", &[1.0, -0.5]),
    ("DIVIDE", &[0.5, -0.25, 0.0, -0.25, 0.0, -0.25]),
    ("DIVIDE2", &[0.25, -0.125, 0.0, -0.125, 0.0, -0.125]),
    ("DIVIDEX2", &[1.0, -0.5, 0.0, -0.5, 0.0, -0.5]),
    ("DOT", &[0.0, -0.25]),
    ("DOT2", &[0.0, -0.125]),
    ("DOTX2", &[0.0, -0.5]),
    ("HIDDEN", &[0.25, -0.125]),
    ("HIDDEN2", &[0.125, -0.0625]),
    ("HIDDENX2", &[0.5, -0.25]),
    ("PHANTOM", &[1.25, -0.25, 0.25, -0.25, 0.25, -0.25]),
    ("PHANTOM2", &[0.625, -0.125, 0.125, -0.125, 0.125, -0.125]),
    ("PHANTOMX2", &[2.5, -0.5, 0.5, -0.5, 0.5, -0.5]),
];

/// The ISO totals from the reference guide.
const ISO_TOTALS: &[(&str, f64)] = &[
    ("ACAD_ISO02W100", 15.0),
    ("ACAD_ISO03W100", 30.0),
    ("ACAD_ISO04W100", 30.0),
    ("ACAD_ISO05W100", 33.0),
    ("ACAD_ISO06W100", 36.0),
    ("ACAD_ISO07W100", 3.0),
    ("ACAD_ISO08W100", 36.0),
    ("ACAD_ISO09W100", 45.0),
    ("ACAD_ISO10W100", 18.0),
    ("ACAD_ISO11W100", 33.0),
    ("ACAD_ISO12W100", 21.0),
    ("ACAD_ISO13W100", 36.0),
    ("ACAD_ISO14W100", 24.0),
    ("ACAD_ISO15W100", 39.0),
];

fn dash() -> Vec<StrokeElement> {
    vec![StrokeElement::Dash(2.0), StrokeElement::Gap(1.0)]
}

#[test]
fn the_library_has_24_families_and_14_iso_patterns_with_unique_names() {
    assert_eq!(stock::LIBRARY.len(), 24 + 14);
    let mut names: Vec<String> = stock::LIBRARY
        .iter()
        .map(|s| s.name.to_ascii_uppercase())
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), stock::LIBRARY.len());
}

#[test]
fn metric_families_are_the_imperial_numbers_times_25_4() {
    assert_eq!(IMPERIAL.len(), 24);
    for (name, imperial) in IMPERIAL {
        let metric = stock::find(name).expect(name).pattern;
        assert_eq!(metric.len(), imperial.len(), "{name}");
        for (m, i) in metric.iter().zip(*imperial) {
            assert!((m - i * 25.4).abs() < 1e-9, "{name}: {m} vs {i} x 25.4");
        }
    }
}

#[test]
fn center2_is_not_a_half_size_center() {
    let center = stock::find("CENTER").unwrap().pattern;
    let center2 = stock::find("CENTER2").unwrap().pattern;
    assert!((center2[0] - center[0] / 2.0).abs() > 1.0);
}

#[test]
fn iso_totals_match_the_guide() {
    for (name, total) in ISO_TOTALS {
        let sum: f64 = stock::find(name)
            .expect(name)
            .pattern
            .iter()
            .map(|v| v.abs())
            .sum();
        assert!((sum - total).abs() < 1e-9, "{name}: {sum} vs {total}");
    }
}

#[test]
fn every_stock_pattern_makes_a_valid_record() {
    let mut table = LinetypeTable::default();
    for s in stock::LIBRARY {
        let id = table.load_stock(s.name).unwrap();
        let record = table.get(id).unwrap();
        assert_eq!(record.elements.len(), s.pattern.len());
        assert!(record.length() > 0.0, "{}", s.name);
    }
}

#[test]
fn a_new_drawing_has_the_special_records_and_the_seeded_names() {
    let table = LinetypeTable::default();
    for (id, name) in [
        (LinetypeId::CONTINUOUS, "CONTINUOUS"),
        (LinetypeId::BYLAYER, "BYLAYER"),
        (LinetypeId::BYBLOCK, "BYBLOCK"),
    ] {
        let r = table.get(id).unwrap();
        assert_eq!(r.name, name);
        assert!(r.is_continuous() && r.length() == 0.0);
        assert!(id.is_special());
    }
    for name in ["DASHED", "DOT", "DASHDOT", "CENTER", "HIDDEN", "PHANTOM"] {
        assert!(table.named(name).is_some(), "{name}");
    }
    assert_eq!(table.records().len(), 9);
    assert!(table.is_stock());
    assert_eq!(table.next_id(), 10);
}

#[test]
fn names_are_found_ignoring_case() {
    let table = LinetypeTable::default();
    assert_eq!(table.named("dashed").unwrap().name, "DASHED");
    assert!(table.named("Dotted").is_none());
}

#[test]
fn adding_gives_fresh_ids_and_refuses_bad_names() {
    let mut table = LinetypeTable::default();
    let a = table.add("Mine", "my line", dash()).unwrap();
    let b = table.add("Other", "", dash()).unwrap();
    assert_ne!(a, b);
    assert!(!table.is_stock());
    assert_eq!(
        table.add("mine", "", dash()),
        Err(LinetypeError::DuplicateName("mine".into()))
    );
    assert_eq!(table.add("  ", "", dash()), Err(LinetypeError::EmptyName));
    assert_eq!(
        table.add(&"x".repeat(256), "", dash()),
        Err(LinetypeError::NameTooLong)
    );
    assert_eq!(
        table.add("ByLayer", "", dash()),
        Err(LinetypeError::DuplicateName("ByLayer".into()))
    );
}

#[test]
fn bad_lengths_are_refused_and_nothing_is_added() {
    let mut table = LinetypeTable::default();
    for bad in [
        StrokeElement::Dash(0.0),
        StrokeElement::Gap(-1.0),
        StrokeElement::Dash(f64::NAN),
        StrokeElement::Gap(f64::INFINITY),
    ] {
        assert_eq!(
            table.add("Bad", "", vec![bad]),
            Err(LinetypeError::BadElement)
        );
    }
    assert!(table.is_stock());
}

#[test]
fn deleted_ids_are_not_reused() {
    let mut table = LinetypeTable::default();
    let a = table.add("A", "", dash()).unwrap();
    table.remove(a).unwrap();
    let b = table.add("B", "", dash()).unwrap();
    assert!(b.0 > a.0);
}

#[test]
fn the_special_records_cannot_change_or_go() {
    let mut table = LinetypeTable::default();
    assert_eq!(
        table.remove(LinetypeId::BYBLOCK),
        Err(LinetypeError::Special("BYBLOCK".into()))
    );
    let mut continuous = table.get(LinetypeId::CONTINUOUS).unwrap().clone();
    continuous.elements = dash();
    assert!(matches!(
        table.replace(continuous),
        Err(LinetypeError::Special(_))
    ));
}

#[test]
fn replacing_keeps_the_id_and_checks_the_name_against_the_others() {
    let mut table = LinetypeTable::default();
    let id = table.add("A", "", dash()).unwrap();
    let mut changed = table.get(id).unwrap().clone();
    changed.name = "Renamed".into();
    let old = table.replace(changed).unwrap();
    assert_eq!(old.name, "A");
    assert_eq!(table.get(id).unwrap().name, "Renamed");

    let mut clash = table.get(id).unwrap().clone();
    clash.name = "dashed".into();
    assert!(matches!(
        table.replace(clash),
        Err(LinetypeError::DuplicateName(_))
    ));
}

#[test]
fn pushing_a_record_with_a_used_id_is_refused() {
    let mut table = LinetypeTable::default();
    let mut r = table.get(LinetypeId(4)).unwrap().clone();
    r.name = "Copy".into();
    assert_eq!(table.push(r), Err(LinetypeError::DuplicateId(4)));
}

#[test]
fn loading_stock_twice_adds_it_once() {
    let mut table = LinetypeTable::default();
    let a = table.load_stock("hidden2").unwrap();
    let b = table.load_stock("HIDDEN2").unwrap();
    assert_eq!(a, b);
    assert_eq!(table.records().len(), 10);
    assert_eq!(table.get(a).unwrap().name, "HIDDEN2");
    assert!(matches!(
        table.load_stock("NOPE"),
        Err(LinetypeError::Unknown(_))
    ));
}

#[test]
fn dxf_values_become_dashes_gaps_and_dots_and_back() {
    for v in [3.5, -1.25, 0.0] {
        assert_eq!(StrokeElement::from_dxf(v).unwrap().to_dxf(), v);
    }
    assert_eq!(StrokeElement::from_dxf(f64::NAN), None);
    assert_eq!(StrokeElement::from_dxf(-2.0), Some(StrokeElement::Gap(2.0)));
}

#[test]
fn length_sums_absolute_values_and_dots_add_nothing() {
    let table = LinetypeTable::default();
    let dashdot = table.named("DASHDOT").unwrap();
    assert!((dashdot.length() - (12.7 + 6.35 + 6.35)).abs() < 1e-9);
}

#[test]
fn lin_rules_are_checked_apart_from_what_a_dxf_may_hold() {
    let mut table = LinetypeTable::default();
    let id = table
        .add(
            "Gap first",
            "",
            vec![StrokeElement::Gap(1.0), StrokeElement::Dash(1.0)],
        )
        .unwrap();
    assert!(table.get(id).unwrap().lin_problem().is_some());
    let one = table
        .add("One", "", vec![StrokeElement::Dash(1.0)])
        .unwrap();
    assert!(table.get(one).unwrap().lin_problem().is_some());
    assert_eq!(table.named("DASHED").unwrap().lin_problem(), None);
}

#[test]
fn a_table_from_records_needs_the_special_ones() {
    let table = LinetypeTable::default();
    let records = table.records().to_vec();
    let back = LinetypeTable::from_records(records.clone(), 20).unwrap();
    assert_eq!(back.records(), table.records());
    assert_eq!(back.next_id(), 20);

    let without: Vec<_> = records
        .iter()
        .filter(|r| r.id != LinetypeId::BYLAYER)
        .cloned()
        .collect();
    assert_eq!(
        LinetypeTable::from_records(without, 10),
        Err(LinetypeError::BadSpecial("BYLAYER"))
    );

    let mut patterned = records;
    patterned[0].elements = dash();
    assert_eq!(
        LinetypeTable::from_records(patterned, 10),
        Err(LinetypeError::BadSpecial("CONTINUOUS"))
    );
}
