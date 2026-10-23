//! Starting a drawing from a template, and saving one. A template is only
//! read: what is made from it is a new drawing with no file, which is not
//! counted as changed until it is edited.

use od_commands::Session;
use od_core::Document;

use crate::io::Done;
use crate::state::AppState;
use crate::templates::{Source, Template};

impl AppState {
    /// A new drawing from `template`: a starter at once, a file after it has
    /// been read in the background.
    pub fn new_from_template(&mut self, template: &Template) {
        match &template.source {
            Source::Starter(starter) => self.start_from(starter.document()),
            Source::File(path) => {
                let path = path.clone();
                let name = template.name.clone();
                self.message = format!("Opening template {name}...");
                self.submit(move || Done::FromTemplate {
                    name,
                    result: Box::new(od_files::load(&path)),
                });
            }
        }
    }

    /// A template file has been read.
    pub(crate) fn finish_template(
        &mut self,
        name: &str,
        result: Result<od_files::Loaded, od_files::FileError>,
    ) {
        match result {
            Ok(loaded) => {
                for warning in &loaded.warnings {
                    self.cmdline.say(warning.clone());
                }
                self.start_from(loaded.document);
            }
            Err(error) => self.tell(format!("Could not use template {name}: {error}")),
        }
    }

    /// Show `document` as a new untitled drawing.
    fn start_from(&mut self, document: Document) {
        self.abort_command();
        let title = format!("Drawing{}", self.next_doc_number);
        self.next_doc_number += 1;
        let doc = self.install_document(title, Session::with_document(document));
        doc.view.request_zoom_extents();
    }

    /// Ask where, and write a copy of the active drawing there as a template
    /// (starting in the templates folder). The drawing itself stays as it is.
    pub fn save_as_template(&mut self) -> bool {
        let doc = &self.docs[self.active];
        let suggested = format!("{}.{}", doc.title, od_files::TEMPLATE_EXTENSION);
        let directory = self.templates_dir.clone().inspect(|d| {
            let _ = std::fs::create_dir_all(d);
        });
        let Some(chosen) = self.picker.save(
            "Save as template",
            &suggested,
            directory.as_deref(),
            od_files::template_filter(),
        ) else {
            return false;
        };
        let path = od_files::template_path(&chosen);
        self.begin_export(&path);
        true
    }
}

#[cfg(test)]
#[path = "template_state_tests.rs"]
mod tests;
