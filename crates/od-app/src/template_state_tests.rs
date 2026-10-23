use std::path::PathBuf;

use od_core::{Entity, Point};

use super::*;
use crate::picker::{Script, ScriptedPicker};
use crate::templates::{Source, Starter};

struct Area(PathBuf);

impl Area {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("od-tplstate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Area(dir)
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn starter(which: Starter) -> Template {
    Template {
        name: which.name().to_owned(),
        source: Source::Starter(which),
        summary: Ok(String::new()),
    }
}

fn file_template(path: PathBuf) -> Template {
    Template {
        name: "plan".to_owned(),
        source: Source::File(path),
        summary: Ok(String::new()),
    }
}

#[test]
fn a_starter_makes_a_new_unsaved_unchanged_drawing_with_its_units() {
    let mut state = AppState::new();
    state.start_on_home();
    state.new_from_template(&starter(Starter::Inches));
    assert!(!state.home);
    let doc = &state.docs[state.active];
    assert_eq!(doc.title, "Drawing1");
    assert_eq!(doc.path, None);
    assert!(!doc.is_modified(), "nothing has been done to it yet");
    assert_eq!(doc.session.document().units().insert.label(), "Inches");
}

#[test]
fn a_file_template_is_read_and_never_changed() {
    let area = Area::new("file");
    let path = area.0.join("plan.odt");
    let mut document = Starter::Millimetres.document();
    document.add(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
    });
    od_files::save(&path, &document).unwrap();
    let before = std::fs::read(&path).unwrap();

    let mut state = AppState::new();
    state.start_on_home();
    state.new_from_template(&file_template(path.clone()));
    state.finish_io();
    let doc = &state.docs[state.active];
    assert_eq!(doc.session.document().len(), 1);
    assert_eq!(
        doc.path, None,
        "saving it asks for a name; the template is safe"
    );
    assert!(!doc.is_modified());
    // Editing and saving the new drawing leaves the template alone.
    state.add_entity(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
    });
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // Starting again makes a second, separate drawing.
    state.new_from_template(&file_template(path));
    state.finish_io();
    assert_eq!(state.docs.len(), 2);
    assert_eq!(state.docs[1].title, "Drawing2");
    assert_eq!(state.docs[1].session.document().len(), 1);
}

#[test]
fn a_template_that_cannot_be_read_says_so_and_makes_nothing() {
    let area = Area::new("broken");
    let path = area.0.join("broken.odt");
    std::fs::write(&path, "not a drawing").unwrap();
    let mut state = AppState::new();
    state.start_on_home();
    state.new_from_template(&file_template(path));
    state.finish_io();
    assert!(
        state.message.starts_with("Could not use template plan"),
        "{}",
        state.message
    );
    assert!(state.home);
    assert!(!state.has_drawings());
}

#[test]
fn saving_as_a_template_writes_a_copy_in_the_templates_folder_and_changes_nothing_else() {
    let area = Area::new("save");
    let folder = area.0.join("templates");
    let mut state = AppState::new();
    state.templates_dir = Some(folder.clone());
    let script = Script::default();
    state.picker = Box::new(ScriptedPicker(script.clone()));
    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 1.0),
    });
    // Cancelling writes nothing.
    assert!(!state.save_as_template());
    assert_eq!(script.titles(), ["Save as template"]);
    assert_eq!(script.suggested(), ["Drawing1.odt"]);
    assert_eq!(script.directories(), [Some(folder.clone())]);
    // A name without the extension gets it.
    script.will_save_to(Some(folder.join("site")));
    assert!(state.save_as_template());
    state.finish_io();
    let saved = folder.join("site.odt");
    assert!(saved.is_file(), "the folder was made and the file written");
    assert_eq!(od_files::load(&saved).unwrap().document.len(), 1);
    // The drawing is still the unsaved Drawing1.
    let doc = &state.docs[state.active];
    assert_eq!(doc.path, None);
    assert!(doc.is_modified());
    // And it is found as a template.
    let found = crate::templates::scan(&folder);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "site");
}
