//! PLOT: write the layout being worked in as a PDF or SVG file. The geometry
//! is `od_core::plot` and the writers are in `od_files`; the file is written
//! off the UI thread like an export, from a copy of the drawing.

use od_core::plot::PlotSpace;

use crate::state::AppState;

/// The file formats of OpenDraft's own plot devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotFormat {
    Pdf,
    Svg,
}

impl PlotFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Svg => "svg",
        }
    }

    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Pdf => od_files::pdf_filter(),
            Self::Svg => od_files::svg_filter(),
        }
    }

    /// `path` with this format's extension: kept if it has it, else added.
    pub fn named(self, path: std::path::PathBuf) -> std::path::PathBuf {
        let has = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(self.extension()));
        if has {
            return path;
        }
        let mut text = path.into_os_string();
        text.push(".");
        text.push(self.extension());
        text.into()
    }
}

impl AppState {
    /// Ask where to write the current layout (or the Model tab) as `format` and start plotting
    /// it, with `draft` (the Plot dialog's page setup) in place of the layout's
    /// own if given. Returns whether a plot was started (not when there is no
    /// layout or the file dialog was cancelled).
    pub fn plot_layout_with(
        &mut self,
        draft: Option<&crate::layouts::PageDraft>,
        format: PlotFormat,
    ) -> bool {
        let doc = &self.docs[self.active];
        let space = draft.map_or_else(
            || {
                doc.view
                    .space()
                    .layout()
                    .map_or(PlotSpace::Model, PlotSpace::Layout)
            },
            |d| d.space,
        );
        let name = match space {
            PlotSpace::Model => Some("Model".to_owned()),
            PlotSpace::Layout(id) => doc.session.document().layout(id).map(|l| l.name.clone()),
        };
        let Some(name) = name else {
            self.message = "That layout is gone".to_owned();
            return false;
        };
        let suggested = format!("{}-{}.{}", doc.title, name, format.extension());
        let directory = self.folder_for_dialogs(doc.path.as_deref());
        let Some(chosen) = self.picker.save(
            "Plot to file",
            &suggested,
            directory.as_deref(),
            format.filter(),
        ) else {
            return false;
        };
        let path = format.named(chosen);
        self.refresh_fields(self.active, crate::field_refresh::ON_PLOT);
        self.begin_plot_with(self.active, &path, space, draft);
        true
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::PlotFormat;

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
        assert!(state.plot_layout_with(None, PlotFormat::Pdf));
        assert_eq!(script.suggested(), ["Drawing1-Layout1.pdf"]);
        assert_eq!(script.filters(), ["PDF file"]);
        let bytes = std::fs::read(dir.join("sheet.pdf")).expect("the extension was added");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(
            String::from_utf8_lossy(&bytes).contains(" l\n"),
            "the model line is on the page"
        );
        assert!(state.message.starts_with("Exported"));
        assert!(state.docs[0].is_modified() && state.docs[0].path.is_none());

        script.will_save_to(Some(dir.join("sheet.svg")));
        assert!(state.plot_layout_with(None, PlotFormat::Svg));
        assert_eq!(script.filters(), ["PDF file", "SVG file"]);
        let svg = std::fs::read_to_string(dir.join("sheet.svg")).unwrap();
        assert!(svg.contains("<polyline"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_dialog_plots_nothing_from_either_tab() {
        let (mut state, _, dir) = state_in_a_layout("needs");
        assert!(!state.plot_layout_with(None, PlotFormat::Pdf), "cancelled");
        state.enter_model();
        assert!(!state.plot_layout_with(None, PlotFormat::Pdf), "cancelled");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod format_tests {
    use super::PlotFormat;
    use std::path::PathBuf;

    #[test]
    fn a_name_gets_the_formats_extension_unless_it_has_it() {
        assert_eq!(
            PlotFormat::Pdf.named(PathBuf::from("a")),
            PathBuf::from("a.pdf")
        );
        assert_eq!(
            PlotFormat::Pdf.named(PathBuf::from("a.PDF")),
            PathBuf::from("a.PDF")
        );
        assert_eq!(
            PlotFormat::Svg.named(PathBuf::from("a.pdf")),
            PathBuf::from("a.pdf.svg")
        );
        assert_eq!(
            PlotFormat::Svg.named(PathBuf::from("plan.v2")),
            PathBuf::from("plan.v2.svg")
        );
    }
}
