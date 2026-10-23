use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("od-templates-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_starters_set_up_their_units() {
    let all = starters();
    assert_eq!(all.len(), 3);
    let mm = Starter::Millimetres.document();
    assert_eq!(mm.units().insert.label(), "Millimeters");
    let feet = Starter::FeetAndInches.document();
    assert_eq!(feet.units().length, LengthFormat::Architectural);
    for template in &all {
        let summary = template.summary.as_ref().unwrap();
        assert!(summary.contains("model space only"), "{summary}");
    }
    assert!(all[0]
        .summary
        .as_ref()
        .unwrap()
        .starts_with("Decimal, millimeters"));
    assert!(all[2]
        .summary
        .as_ref()
        .unwrap()
        .starts_with("Architectural, inches"));
}

#[test]
fn a_starter_is_a_new_document_every_time() {
    let mut first = Starter::Inches.document();
    first.add(od_core::Entity::Circle {
        center: od_core::Point::new(0.0, 0.0),
        radius: 1.0,
    });
    assert!(
        Starter::Inches.document().is_empty(),
        "nothing leaks between drawings"
    );
}

#[test]
fn the_folder_lists_templates_by_name_and_says_which_cannot_be_read() {
    let dir = scratch("scan");
    let mut doc = Starter::Millimetres.document();
    doc.add(od_core::Entity::Line {
        start: od_core::Point::new(0.0, 0.0),
        end: od_core::Point::new(1.0, 1.0),
    });
    od_files::save(&dir.join("b layout.odt"), &doc).unwrap();
    od_files::save(&dir.join("A frame.odt"), &doc).unwrap();
    std::fs::write(dir.join("broken.odt"), "not a drawing").unwrap();
    // Other files are not templates.
    od_files::save(&dir.join("plan.odx"), &doc).unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    let found = scan(&dir);
    let names: Vec<&str> = found.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["A frame", "b layout", "broken"]);
    assert!(found[0].summary.is_ok());
    assert!(found[2].summary.is_err(), "unavailable, with a reason");
    assert!(!found[2].summary.as_ref().unwrap_err().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_folder_has_no_templates() {
    assert!(scan(Path::new("no/such/templates")).is_empty());
}
