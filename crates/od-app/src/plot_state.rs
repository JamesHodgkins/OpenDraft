//! PLOT: write the layout being worked in as a PDF or SVG file. The geometry
//! is `od_core::plot` and the writers are in `od_files`; the file is written
//! off the UI thread like an export, from a copy of the drawing.

use crate::state::AppState;

impl AppState {
    /// Ask where to write the current layout and start plotting it. Returns
    /// whether a plot was started (not when there is no layout or the dialog
    /// was cancelled).
    pub fn plot_layout(&mut self) -> bool {
        let doc = &self.docs[self.active];
        let Some(layout) = doc
            .view
            .space()
            .layout()
            .and_then(|l| doc.session.document().layout(l))
        else {
            self.message = "PLOT plots a layout: switch to one first".to_owned();
            return false;
        };
        let (id, suggested) = (layout.id, format!("{}-{}.pdf", doc.title, layout.name));
        let directory = self.folder_for_dialogs(doc.path.as_deref());
        let Some(chosen) = self.picker.save(
            "Plot layout to file",
            &suggested,
            directory.as_deref(),
            od_files::plot_filter(),
        ) else {
            return false;
        };
        let known = chosen
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["pdf", "svg"].contains(&e.to_ascii_lowercase().as_str()));
        let path = if known {
            chosen
        } else {
            chosen.with_extension("pdf")
        };
        self.begin_plot(&path, id);
        true
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use crate::picker::{Script, ScriptedPicker};
    use crate::state::AppState;

    fn state_in_a_layout(name: &str) -> (AppState, Script, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("od-plot-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = AppState::new();
        let script = Script::default();
        state.picker = Box::new(ScriptedPicker(script.clone()));
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1000.0, 500.0),
        });
        state.new_layout();
        (state, script, dir)
    }

    #[test]
    fn plotting_writes_a_pdf_and_leaves_the_drawing_untouched() {
        let (mut state, script, dir) = state_in_a_layout("writes");
        script.will_save_to(Some(dir.join("sheet")));
        assert!(state.plot_layout());
        assert_eq!(script.suggested(), ["Drawing1-Layout1.pdf"]);
        assert_eq!(script.filters(), ["Plot file (PDF, SVG)"]);
        let bytes = std::fs::read(dir.join("sheet.pdf")).expect("the extension was added");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(
            String::from_utf8_lossy(&bytes).contains(" l\n"),
            "the model line is on the page"
        );
        assert!(state.message.starts_with("Exported"));
        assert!(state.docs[0].is_modified() && state.docs[0].path.is_none());

        script.will_save_to(Some(dir.join("sheet.svg")));
        assert!(state.plot_layout());
        let svg = std::fs::read_to_string(dir.join("sheet.svg")).unwrap();
        assert!(svg.contains("<polyline"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plot_needs_a_layout_and_a_cancelled_dialog_plots_nothing() {
        let (mut state, _, dir) = state_in_a_layout("needs");
        assert!(!state.plot_layout(), "cancelled");
        state.enter_model();
        assert!(!state.plot_layout());
        assert!(state.message.starts_with("PLOT plots a layout"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
