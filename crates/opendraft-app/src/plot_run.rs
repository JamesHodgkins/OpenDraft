//! Carrying out a finished -PLOT: the command only asks; the app writes the
//! file or sends the job, as the Plot dialog does.

use std::sync::Arc;

use crate::plot_job::document_with_draft;
use crate::plot_line::{plot_prompt, Destination, PlotRequest, Setup};
use crate::state::AppState;

impl AppState {
    /// Start -PLOT on the drawing being worked in.
    pub fn start_plot_line(&mut self) {
        let doc = &self.docs[self.active];
        let setup = Setup {
            requests: Arc::clone(&self.command_dialogs.plot_requests),
            backend: self.command_dialogs.print.0.clone(),
            folder: self.folder_for_dialogs(doc.path.as_deref()),
            title: doc.title.clone(),
            last_device: self.settings.plot_device.clone(),
            dpi: self.settings.plot_dpi,
        };
        self.start_command("-PLOT", move |ed| plot_prompt(ed, setup));
    }

    /// Do what finished -PLOT commands asked. Waits until the command has
    /// ended, so that fields are brought up to date like any other plot.
    pub(crate) fn take_plot_requests(&mut self) {
        if self.runner.is_some() {
            return;
        }
        let requests = match self.command_dialogs.plot_requests.lock() {
            Ok(mut list) => std::mem::take(&mut *list),
            Err(_) => return,
        };
        for request in requests {
            self.run_plot_request(request);
        }
    }

    fn run_plot_request(&mut self, request: PlotRequest) {
        let PlotRequest {
            draft,
            destination,
            save_changes,
        } = request;
        if save_changes {
            if let Err(why) = self.set_page_setup(&draft) {
                self.tell(format!("The page setup was not saved: {why}"));
                return;
            }
        }
        if let Err(why) = document_with_draft(self.session().document(), &draft) {
            self.tell(format!("Nothing was plotted: {why}"));
            return;
        }
        self.settings.plot_device = draft.plot.device_name.clone();
        match destination {
            Destination::File(path) => {
                self.refresh_fields(self.active, crate::field_refresh::ON_PLOT);
                self.begin_plot_with(self.active, &path, draft.space, Some(&draft));
            }
            Destination::Printer {
                printer,
                copies,
                dpi,
            } => {
                self.settings.plot_dpi = dpi;
                self.print_draft(&draft, printer, copies, dpi);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use crate::picker::{Script, ScriptedPicker};
    use crate::print_backend::fake::FakeBackend;
    use crate::state::AppState;

    struct Run {
        state: AppState,
        dir: std::path::PathBuf,
        fake: Arc<FakeBackend>,
    }

    use std::sync::Arc;

    fn run_in_layout() -> Run {
        let name = std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-");
        let dir =
            std::env::temp_dir().join(format!("opendraft-plot-line-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = AppState::new();
        state.picker = Box::new(ScriptedPicker(Script::default()));
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1000.0, 500.0),
        });
        state.new_layout();
        let fake = Arc::new(FakeBackend::office());
        state.command_dialogs.print = crate::dialogs::PrintHandle(fake.clone());
        Run { state, dir, fake }
    }

    fn answer(state: &mut AppState, text: &str) {
        state.settle_command();
        if text.is_empty() {
            state.answer_enter();
        } else {
            assert!(state.answer_typed(text), "{text}");
        }
        state.settle_command();
    }

    #[test]
    fn a_simple_plot_to_a_file_asks_five_things_and_writes_the_pdf() {
        let mut run = run_in_layout();
        let file = run.dir.join("out");
        run.state.start_plot_line();
        for reply in ["", "", "pdf", file.to_str().unwrap(), "", ""] {
            answer(&mut run.state, reply);
        }
        let bytes = std::fs::read(run.dir.join("out.pdf")).expect("the extension was added");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(!run.state.command_running());
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn an_area_that_cannot_be_plotted_is_told_and_asked_again() {
        let mut run = run_in_layout();
        run.state.start_plot_line();
        for reply in ["Yes", "", "PDF", "A3", "", "Landscape", "", "Display"] {
            answer(&mut run.state, reply);
        }
        assert!(run.state.command_running());
        let prompt = run.state.runner_prompt().unwrap().plain();
        assert!(prompt.contains("plot area"), "{prompt}");
        run.state.cancel_command();
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn plot_can_choose_the_model_tab_by_name() {
        let mut run = run_in_layout();
        let file = run.dir.join("model");
        run.state.start_plot_line();
        for reply in ["", "model", "pdf", file.to_str().unwrap(), "", ""] {
            answer(&mut run.state, reply);
        }
        let bytes = std::fs::read(run.dir.join("model.pdf")).expect("the model was plotted");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn the_svg_device_writes_an_svg_and_is_remembered() {
        let mut run = run_in_layout();
        let file = run.dir.join("out");
        run.state.start_plot_line();
        for reply in ["", "", "od svg", file.to_str().unwrap(), "", ""] {
            answer(&mut run.state, reply);
        }
        let svg = std::fs::read_to_string(run.dir.join("out.svg")).expect("an svg");
        assert!(svg.contains("<svg"));
        assert_eq!(run.state.settings.plot_device, "OpenDraft SVG");
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn a_plot_to_a_printer_sends_the_copies_and_leaves_the_layout_alone() {
        let mut run = run_in_layout();
        let before = run.state.session().document().layouts()[0].plot.clone();
        run.state.start_plot_line();
        for reply in ["", "", "office", "2", "", ""] {
            answer(&mut run.state, reply);
        }
        let jobs = run.fake.jobs.lock().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].copies, jobs[0].printer.0.as_str()), (2, "Office"));
        assert_eq!(run.state.session().document().layouts()[0].plot, before);
        assert_eq!(run.state.settings.plot_device, "Office");
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn a_wrong_name_is_told_and_asked_again_and_escape_plots_nothing() {
        let mut run = run_in_layout();
        run.state.start_plot_line();
        answer(&mut run.state, "");
        answer(&mut run.state, "Nowhere");
        assert!(run.state.command_running(), "asked for the layout again");
        run.state.cancel_command();
        assert!(run.fake.jobs.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&run.dir);
    }

    #[test]
    fn a_detailed_plot_with_save_changes_writes_the_setup_in_one_undo_step() {
        let mut run = run_in_layout();
        let file = run.dir.join("d.pdf");
        run.state.start_plot_line();
        let replies = [
            "Yes",
            "",
            "PDF", // detailed, layout, device
            "A3",
            "",
            "Landscape",
            "", // paper, units, orientation, upside down
            "Extents",
            "1:50",
            "5,5", // area, scale, offset
            "",
            "",
            "",
            "", // lineweights x2, order, borders (no style questions)
            file.to_str().unwrap(),
            "Yes",
            "",
        ];
        for reply in replies {
            answer(&mut run.state, reply);
        }
        assert!(file.exists());
        let plot = run.state.session().document().layouts()[0].plot.clone();
        assert_eq!(plot.canonical_media_name, "A3");
        assert_eq!(plot.area, opendraft_core::PlotArea::Extents);
        assert_eq!(plot.origin_mm, Point::new(5.0, 5.0));
        run.state.undo();
        assert_eq!(
            run.state.session().document().layouts()[0].plot.area,
            opendraft_core::PlotArea::Layout
        );
        let _ = std::fs::remove_dir_all(&run.dir);
    }
}
