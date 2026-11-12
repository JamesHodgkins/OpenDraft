//! The Plot dialog's state and what its buttons do. The dialog edits a
//! complete copy of the layout's page setup; nothing reaches the drawing until
//! "Apply to Layout" (or "Save changes to layout" with Plot), and a plot that
//! only borrows the draft never touches the drawing's history.

use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use opendraft_core::plot::{plot_sheet, PlotSpace};
use opendraft_core::plot_plan::validate_plot;
use opendraft_core::plot_settings::{DEVICE_PDF, DEVICE_SVG};
use opendraft_core::Document;

use crate::layouts::PageDraft;
use crate::plot_state::PlotFormat;
use crate::print_backend::{PrintError, PrintJob, Printer, PrinterId};
use crate::state::AppState;

/// How long the settings stay still before the preview is drawn again.
pub const PREVIEW_DELAY: Duration = Duration::from_millis(250);
/// Width of the preview picture in pixels.
const PREVIEW_WIDTH: u32 = 720;
/// The tallest preview picture, in pixels.
const PREVIEW_MAX_HEIGHT: u32 = 1600;

/// What the sheet looks like with the draft applied: the exact sheet the
/// plot would write, drawn on white.
#[derive(Clone, Debug)]
pub struct PlotPreview {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
    /// What the plot could not show faithfully.
    pub notes: Vec<String>,
}

/// What the printers of the machine are, found off the UI thread.
pub enum Printers {
    Asking(Receiver<Result<Vec<Printer>, PrintError>>),
    Ready(Vec<Printer>),
    Failed(String),
}

/// Where the plot goes: one of OpenDraft's own file devices, or a printer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A PDF file, asked for when Plot is pressed.
    Pdf,
    /// An SVG file, asked for when Plot is pressed.
    Svg,
    Printer(PrinterId),
}

impl Target {
    /// The device as a page setup names it.
    pub fn device_name(&self) -> String {
        match self {
            Self::Pdf => DEVICE_PDF.to_owned(),
            Self::Svg => DEVICE_SVG.to_owned(),
            Self::Printer(printer) => printer.0.clone(),
        }
    }

    /// OpenDraft's own devices by name; a printer is found among the
    /// machine's once they are known.
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            DEVICE_PDF => Some(Self::Pdf),
            DEVICE_SVG => Some(Self::Svg),
            _ => None,
        }
    }
}

/// The device to start on: the layout's own, else the one used last. A
/// printer is only named (to be found once the printers are known).
pub fn initial_device(layout_device: &str, last: &str) -> (Target, Option<String>) {
    for name in [layout_device, last] {
        if let Some(target) = Target::builtin(name) {
            return (target, None);
        }
        // Not a device: what a layout holds when none was chosen, or the
        // name an earlier version gave to both file devices.
        if !name.is_empty() && name != "none_device" && name != "OpenDraft PDF/SVG" {
            return (Target::Pdf, Some(name.to_owned()));
        }
    }
    (Target::Pdf, None)
}

impl PlotDialog {
    /// Once the printers are known, go back to the one used last time if it
    /// is still there.
    pub fn settle(&mut self) {
        let Printers::Ready(list) = &self.printers else {
            return;
        };
        if let Some(wanted) = self.remembered.take() {
            if let Some(found) = list.iter().find(|p| p.id.0 == wanted) {
                self.target = Target::Printer(found.id.clone());
            }
        }
    }
}

impl Printers {
    /// Takes the answer once it has arrived.
    pub fn poll(&mut self) {
        if let Self::Asking(answer) = self {
            if let Ok(found) = answer.try_recv() {
                *self = match found {
                    Ok(list) => Self::Ready(list),
                    Err(why) => Self::Failed(why.to_string()),
                };
            }
        }
    }
}

pub struct PlotDialog {
    pub draft: PageDraft,
    pub target: Target,
    pub copies: u32,
    /// Resolution of a sheet printed as a picture.
    pub dpi: u32,
    /// The printer used last time, taken once the printers are known.
    pub remembered: Option<String>,
    pub printers: Printers,
    /// Write the draft to the layout when Plot is pressed.
    pub save_changes: bool,
    pub preview: Option<PlotPreview>,
    /// The draft the preview was drawn from; the picture is out of date when it differs.
    pub previewed: Option<PageDraft>,
    /// Set while the dialog is hidden for the plot window to be picked.
    pub picking: Option<crate::plot_pick::WindowSlot>,
    /// A picture being drawn on another thread, with the draft it is of.
    pub drawing: Option<Receiver<(PageDraft, Result<PlotPreview, String>)>>,
    /// The draft as last seen and since when, to wait for edits to settle.
    pub seen: Option<(PageDraft, Instant)>,
    /// Why the last button press did not do its job.
    pub error: Option<String>,
    /// Bumped whenever the preview picture changes, so the window knows to
    /// upload it again.
    pub preview_serial: u64,
}

/// One step of keeping the preview up to date: take a finished picture, and
/// when the settings have changed and then stayed still for [`PREVIEW_DELAY`]
/// (at once for the first picture) draw it again on another thread. Returns
/// whether the window should look again soon.
fn keep_preview_current(dialog: &mut PlotDialog, document: &Document, now: Instant) -> bool {
    if let Some(drawing) = &dialog.drawing {
        if let Ok((draft, result)) = drawing.try_recv() {
            dialog.drawing = None;
            match result {
                Ok(preview) => {
                    dialog.preview = Some(preview);
                    dialog.preview_serial += 1;
                    dialog.error = None;
                }
                Err(why) => dialog.error = Some(why),
            }
            dialog.previewed = Some(draft);
        }
    }
    if dialog.previewed.as_ref() == Some(&dialog.draft) {
        return dialog.drawing.is_some();
    }
    if dialog.seen.as_ref().map(|(d, _)| d) != Some(&dialog.draft) {
        dialog.seen = Some((dialog.draft.clone(), now));
    }
    let since = dialog.seen.as_ref().map_or(now, |(_, at)| *at);
    let settled = dialog.previewed.is_none() || now.duration_since(since) >= PREVIEW_DELAY;
    if dialog.drawing.is_none() && settled {
        let (send, receive) = channel();
        let (document, draft) = (document.clone(), dialog.draft.clone());
        let spawned = std::thread::Builder::new()
            .name("opendraft-plot-preview".into())
            .spawn(move || {
                let result = render_preview(&document, &draft);
                let _ = send.send((draft, result));
            });
        if spawned.is_ok() {
            dialog.drawing = Some(receive);
        } else {
            dialog.previewed = Some(dialog.draft.clone());
        }
    }
    true
}

/// `document` with the setup of `draft` (a layout's or the Model tab's)
/// replaced by it: a copy to plot from, never the open drawing.
pub fn document_with_draft(document: &Document, draft: &PageDraft) -> Result<Document, String> {
    let mut plot = draft.plot.clone();
    plot.paper_width_mm = draft.width;
    plot.paper_height_mm = draft.height;
    validate_plot(&plot).map_err(|e| e.to_string())?;
    let mut copy = document.clone();
    match draft.space {
        PlotSpace::Model => {
            copy.set_model_plot(plot);
        }
        PlotSpace::Layout(id) => {
            let mut layouts = document.layouts().to_vec();
            let layout = layouts
                .iter_mut()
                .find(|l| l.id == id)
                .ok_or("The layout is gone.")?;
            layout.width = draft.width;
            layout.height = draft.height;
            layout.plot = plot;
            copy.set_layouts(layouts);
        }
    }
    Ok(copy)
}

/// The picture of the plot `draft` describes.
pub fn render_preview(document: &Document, draft: &PageDraft) -> Result<PlotPreview, String> {
    let copy = document_with_draft(document, draft)?;
    let sheet = plot_sheet(&copy, draft.space).map_err(|e| e.to_string())?;
    let (width, height) = opendraft_files::png_size(&sheet, PREVIEW_WIDTH);
    if height > PREVIEW_MAX_HEIGHT {
        return Err("The sheet is too tall to preview.".to_owned());
    }
    let bytes = opendraft_files::png(&sheet, width, height).map_err(|e| e.to_string())?;
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut rgba = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("The preview is too large.")?
    ];
    let info = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
    rgba.truncate(info.buffer_size());
    Ok(PlotPreview {
        size: [info.width as usize, info.height as usize],
        rgba,
        notes: sheet.notes,
    })
}

impl AppState {
    /// Open the Plot dialog on the layout being worked in, with its page setup
    /// as the draft. Returns whether it opened.
    pub fn open_plot_dialog(&mut self, preview: bool) -> bool {
        let space = self
            .space()
            .layout()
            .map_or(PlotSpace::Model, PlotSpace::Layout);
        let Some(mut draft) = PageDraft::of(self.session().document(), space) else {
            self.message = "That layout is gone".to_owned();
            return false;
        };
        // A Model tab saved with an area it cannot plot (AutoCAD's own is
        // Display, a DXF may say Layout) opens on Extents, in the draft only.
        let plottable = matches!(
            draft.plot.area,
            opendraft_core::PlotArea::Extents | opendraft_core::PlotArea::Window { .. }
        );
        if draft.space == PlotSpace::Model && !plottable {
            draft.plot.area = opendraft_core::PlotArea::Extents;
        }
        let (target, remembered) =
            initial_device(&draft.plot.device_name, &self.settings.plot_device);
        let backend = self.command_dialogs.print.0.clone();
        let (send, answer) = channel();
        let asked = std::thread::Builder::new()
            .name("od-printers".into())
            .spawn(move || {
                let _ = send.send(backend.printers());
            });
        let printers = match asked {
            Ok(_) => Printers::Asking(answer),
            Err(_) => Printers::Failed("the printers could not be asked".to_owned()),
        };
        self.command_dialogs.plot = Some(PlotDialog {
            target,
            copies: 1,
            dpi: self.settings.plot_dpi,
            remembered,
            printers,
            draft,
            save_changes: true,
            preview: None,
            previewed: None,
            picking: None,
            drawing: None,
            seen: None,
            error: None,
            preview_serial: 0,
        });
        if preview {
            self.plot_dialog_preview();
        }
        true
    }

    /// Draw the dialog's preview from its draft.
    pub fn plot_dialog_preview(&mut self) {
        let Some(mut dialog) = self.command_dialogs.plot.take() else {
            return;
        };
        match render_preview(self.session().document(), &dialog.draft) {
            Ok(preview) => {
                dialog.preview = Some(preview);
                dialog.previewed = Some(dialog.draft.clone());
                dialog.preview_serial += 1;
                dialog.error = None;
            }
            Err(why) => dialog.error = Some(why),
        }
        self.command_dialogs.plot = Some(dialog);
    }

    /// Keep the preview up to date: take a finished picture, and when the
    /// settings have changed and then stayed still for [`PREVIEW_DELAY`]
    /// (at once for the first picture), draw it again on another thread.
    /// Returns whether the window should look again soon.
    pub fn plot_dialog_tick(&mut self, now: Instant) -> bool {
        let Some(mut dialog) = self.command_dialogs.plot.take() else {
            return false;
        };
        let again = keep_preview_current(&mut dialog, self.session().document(), now);
        self.command_dialogs.plot = Some(dialog);
        again
    }

    /// Write the draft to the layout now (one undo step) and keep the dialog.
    pub fn plot_dialog_apply(&mut self) {
        let Some(mut dialog) = self.command_dialogs.plot.take() else {
            return;
        };
        dialog.draft.plot.device_name = dialog.target.device_name();
        dialog.error = self.set_page_setup(&dialog.draft).err();
        self.command_dialogs.plot = Some(dialog);
    }

    /// Plot with the draft: ask for the file, then write it from a copy of
    /// the drawing. With "Save changes to layout" (on unless unticked) the
    /// draft is written to the layout once the plot has started, so the next
    /// plot opens on it and a cancelled one changes nothing.
    /// Closes the dialog when the plot starts.
    pub fn plot_dialog_run(&mut self) -> bool {
        let Some(mut dialog) = self.command_dialogs.plot.take() else {
            return false;
        };
        dialog.draft.plot.device_name = dialog.target.device_name();
        if let Err(why) = document_with_draft(self.session().document(), &dialog.draft) {
            dialog.error = Some(why);
            self.command_dialogs.plot = Some(dialog);
            return false;
        }
        self.settings.plot_device = dialog.target.device_name();
        self.settings.plot_dpi = dialog.dpi;
        let started = match dialog.target.clone() {
            Target::Pdf => self.plot_layout_with(Some(&dialog.draft), PlotFormat::Pdf),
            Target::Svg => self.plot_layout_with(Some(&dialog.draft), PlotFormat::Svg),
            Target::Printer(printer) => {
                self.print_draft(&dialog.draft, printer, dialog.copies, dialog.dpi)
            }
        };
        if started {
            if dialog.save_changes {
                // The plot's own status message stays.
                let message = std::mem::take(&mut self.message);
                self.message = match self.set_page_setup(&dialog.draft) {
                    Ok(()) => message,
                    Err(why) => why,
                };
            }
            return true;
        }
        self.command_dialogs.plot = Some(dialog);
        false
    }
}

impl AppState {
    /// Say how a paper job ended.
    pub(crate) fn finish_print(
        &mut self,
        printer: &str,
        result: Result<crate::print_backend::Receipt, PrintError>,
    ) {
        match result {
            Ok(receipt) => {
                let queue = receipt
                    .queue_id
                    .map(|id| format!(" (job {id})"))
                    .unwrap_or_default();
                self.tell(format!("Sent to {printer}{queue}"));
            }
            Err(PrintError::Cancelled) => self.tell("Printing was cancelled".to_owned()),
            Err(why) => self.tell(format!("Could not print to {printer}: {why}")),
        }
    }

    /// Send the draft's sheet to `printer` from a worker: the sheet is made
    /// from a copy of the drawing, so the drawing is not held up or changed.
    pub(crate) fn print_draft(
        &mut self,
        draft: &PageDraft,
        printer: PrinterId,
        copies: u32,
        dpi: u32,
    ) -> bool {
        let index = self.active;
        self.refresh_fields(index, crate::field_refresh::ON_PLOT);
        let Ok(copy) = document_with_draft(self.docs[index].drawing().document(), draft) else {
            return false;
        };
        let space = draft.space;
        let backend = self.command_dialogs.print.0.clone();
        let name = printer.0.clone();
        self.message = format!("Printing to {name}...");
        self.submit_cancellable(move |cancel| {
            let result = plot_sheet(&copy, space)
                .map_err(|e| PrintError::Plot(e.to_string()))
                .and_then(|sheet| {
                    let job = PrintJob {
                        printer,
                        copies,
                        dpi,
                        sheet,
                    };
                    backend.submit(&job, cancel)
                });
            crate::io::Done::Printed {
                printer: name,
                result,
            }
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, PlotArea, PlotScale, Point, StandardScale};

    use crate::picker::{Script, ScriptedPicker};
    use crate::state::AppState;

    fn state() -> (AppState, Script, std::path::PathBuf) {
        let name = std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-");
        let dir =
            std::env::temp_dir().join(format!("opendraft-plot-job-{name}-{}", std::process::id()));
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
    fn the_dialog_opens_on_the_layouts_setup_or_the_model_tabs() {
        let (mut state, _, dir) = state();
        assert!(state.open_plot_dialog(false));
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert_eq!(
            dialog.draft.plot,
            state.session().document().layouts()[0].plot
        );
        state.command_dialogs.plot = None;
        state.enter_model();
        assert!(state.open_plot_dialog(false));
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert_eq!(dialog.draft.space, opendraft_core::plot::PlotSpace::Model);
        assert_eq!(&dialog.draft.plot, state.session().document().model_plot());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_model_tab_that_cannot_plot_its_area_opens_on_extents_without_changing_the_drawing() {
        let (mut state, _, dir) = state();
        state.enter_model();
        for saved in [PlotArea::Layout, PlotArea::Display] {
            let mut draft = super::PageDraft::of(
                state.session().document(),
                opendraft_core::plot::PlotSpace::Model,
            )
            .unwrap();
            draft.plot.area = saved.clone();
            state.set_page_setup(&draft).unwrap();
            assert!(state.open_plot_dialog(false));
            let dialog = state.command_dialogs.plot.take().unwrap();
            assert_eq!(dialog.draft.plot.area, PlotArea::Extents);
            assert!(crate::plot_controls::control_state(&dialog.draft.plot, true).can_submit);
            assert_eq!(state.session().document().model_plot().area, saved);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_preview_is_a_picture_of_the_draft_and_changes_nothing() {
        let (mut state, _, dir) = state();
        state.open_plot_dialog(true);
        let revision = state.session().revision();
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        let first = dialog.preview.clone().expect("opened with a preview");
        assert_eq!(first.rgba.len(), first.size[0] * first.size[1] * 4);
        dialog.draft.plot.area = PlotArea::Extents;
        dialog.draft.plot.scale = PlotScale::Standard {
            kind: StandardScale::Fit,
            value: 1.0,
        };
        state.plot_dialog_preview();
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert_ne!(dialog.preview.as_ref().unwrap().rgba, first.rgba);
        assert_eq!(dialog.preview_serial, 2);
        assert_eq!(
            state.session().revision(),
            revision,
            "the drawing is untouched"
        );
        assert_eq!(
            state.session().document().layouts()[0].plot.area,
            PlotArea::Layout
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_preview_follows_the_settings_after_they_settle() {
        let (mut state, _, dir) = state();
        state.open_plot_dialog(false);
        let t0 = std::time::Instant::now();
        let wait = |state: &mut AppState, mut at: std::time::Instant| {
            for _ in 0..500 {
                state.plot_dialog_tick(at);
                if state
                    .command_dialogs
                    .plot
                    .as_ref()
                    .unwrap()
                    .drawing
                    .is_none()
                {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
                at = std::time::Instant::now().max(at);
            }
            panic!("the preview never arrived");
        };
        assert!(
            state.plot_dialog_tick(t0),
            "the first picture is drawn at once"
        );
        wait(&mut state, t0);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        let first = dialog.preview.clone().unwrap();
        assert_eq!(dialog.previewed.as_ref(), Some(&dialog.draft));

        dialog.draft.plot.margins_mm = [20.0; 4];
        state.plot_dialog_tick(t0);
        state.plot_dialog_tick(t0 + super::PREVIEW_DELAY / 2);
        assert!(
            state
                .command_dialogs
                .plot
                .as_ref()
                .unwrap()
                .drawing
                .is_none(),
            "not while it is still being edited"
        );
        let later = t0 + super::PREVIEW_DELAY * 2;
        state.plot_dialog_tick(later);
        assert!(state
            .command_dialogs
            .plot
            .as_ref()
            .unwrap()
            .drawing
            .is_some());
        wait(&mut state, later);
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert_ne!(dialog.preview.as_ref().unwrap().rgba, first.rgba);
        assert_eq!(dialog.previewed.as_ref(), Some(&dialog.draft));
        assert_eq!(dialog.preview_serial, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_draft_is_refused_with_a_reason_and_the_dialog_stays() {
        let (mut state, _, dir) = state();
        state.open_plot_dialog(false);
        state.command_dialogs.plot.as_mut().unwrap().draft.plot.area = PlotArea::Window {
            first: Point::new(0.0, 0.0),
            second: Point::new(0.0, 9.0),
        };
        state.plot_dialog_preview();
        assert!(state.command_dialogs.plot.as_ref().unwrap().error.is_some());
        assert!(!state.plot_dialog_run());
        assert!(state.command_dialogs.plot.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plotting_with_a_draft_leaves_the_layout_alone_when_unticked() {
        let (mut state, script, dir) = state();
        let before = state.session().document().layouts()[0].plot.clone();
        state.open_plot_dialog(false);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.draft.plot.margins_mm = [10.0; 4];
        dialog.save_changes = false;
        script.will_save_to(Some(dir.join("a.pdf")));
        assert!(state.plot_dialog_run());
        assert!(state.command_dialogs.plot.is_none());
        assert!(dir.join("a.pdf").exists(), "{}", state.message);
        assert_eq!(state.session().document().layouts()[0].plot, before);

        state.open_plot_dialog(false);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.draft.plot.margins_mm = [10.0; 4];
        dialog.save_changes = true;
        script.will_save_to(Some(dir.join("b.pdf")));
        assert!(state.plot_dialog_run());
        assert_eq!(
            state.session().document().layouts()[0].plot.margins_mm,
            [10.0; 4]
        );
        state.undo();
        assert_eq!(state.session().document().layouts()[0].plot, before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_is_one_undo_step_and_applying_twice_adds_none() {
        let (mut state, _, dir) = state();
        state.open_plot_dialog(false);
        state
            .command_dialogs
            .plot
            .as_mut()
            .unwrap()
            .draft
            .plot
            .margins_mm = [5.0; 4];
        state.plot_dialog_apply();
        state.plot_dialog_apply();
        assert_eq!(
            state.session().document().layouts()[0].plot.margins_mm,
            [5.0; 4]
        );
        state.undo();
        assert_eq!(
            state.session().document().layouts()[0].plot.margins_mm,
            [0.0; 4]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plot_remembers_its_setup_unless_unticked_or_cancelled() {
        let (mut state, script, dir) = state();
        let margins = |state: &AppState| state.session().document().layouts()[0].plot.margins_mm;
        let change = |state: &mut AppState| {
            let dialog = state.command_dialogs.plot.as_mut().unwrap();
            dialog.draft.plot.margins_mm = [10.0; 4];
            dialog.draft.plot.flags &= !opendraft_core::plot_settings::flags::PLOT_CENTERED;
        };
        let before = margins(&state);
        state.open_plot_dialog(false);
        assert!(state.command_dialogs.plot.as_ref().unwrap().save_changes);
        change(&mut state);
        assert!(!state.plot_dialog_run(), "the picker answers cancelled");
        assert_eq!(margins(&state), before, "a cancelled plot saves nothing");

        state.command_dialogs.plot.as_mut().unwrap().save_changes = false;
        script.will_save_to(Some(dir.join("once.pdf")));
        assert!(state.plot_dialog_run());
        assert_eq!(margins(&state), before, "unticked, it is a one-off plot");

        state.open_plot_dialog(false);
        change(&mut state);
        script.will_save_to(Some(dir.join("kept.pdf")));
        assert!(state.plot_dialog_run());
        assert_eq!(margins(&state), [10.0; 4]);
        state.open_plot_dialog(false);
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert_eq!(
            dialog.draft.plot.margins_mm, [10.0; 4],
            "the next one starts there"
        );
        state.undo();
        assert_eq!(margins(&state), before, "one undo step takes it back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn with_fake(state: &mut AppState) -> std::sync::Arc<crate::print_backend::fake::FakeBackend> {
        let fake = std::sync::Arc::new(crate::print_backend::fake::FakeBackend::office());
        state.command_dialogs.print = crate::dialogs::PrintHandle(fake.clone());
        fake
    }

    /// Wait for the printers to be found, as the dialog's frames would.
    fn printers_of(state: &mut AppState) -> Vec<String> {
        for _ in 0..200 {
            let dialog = state.command_dialogs.plot.as_mut().unwrap();
            dialog.printers.poll();
            if let super::Printers::Ready(list) = &dialog.printers {
                return list.iter().map(|p| p.id.0.clone()).collect();
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the printers never arrived");
    }

    #[test]
    fn the_dialog_finds_printers_and_a_plot_goes_to_the_one_chosen() {
        let (mut state, _, dir) = state();
        let fake = with_fake(&mut state);
        state.open_plot_dialog(false);
        assert_eq!(printers_of(&mut state), ["Office"]);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.target = super::Target::Printer(crate::print_backend::PrinterId("Office".into()));
        dialog.copies = 3;
        let revision = state.session().revision();
        assert!(state.plot_dialog_run());
        let jobs = fake.jobs.lock().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].copies, jobs[0].printer.0.as_str()), (3, "Office"));
        assert!(
            !jobs[0].sheet.paths.is_empty(),
            "the model line is on the sheet"
        );
        assert!(
            state.message.starts_with("Sent to Office"),
            "{}",
            state.message
        );
        assert!(state.session().revision() > revision);
        assert_eq!(
            state.session().document().layouts()[0].plot.device_name,
            "Office",
            "the layout remembers its printer"
        );
        assert!(state.command_dialogs.plot.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_last_printer_and_resolution_come_back_next_time() {
        let (mut state, _, dir) = state();
        with_fake(&mut state);
        state.open_plot_dialog(false);
        printers_of(&mut state);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.target = super::Target::Printer(crate::print_backend::PrinterId("Office".into()));
        dialog.dpi = 600;
        assert!(state.plot_dialog_run());
        assert_eq!(
            (state.settings.plot_device.as_str(), state.settings.plot_dpi),
            ("Office", 600)
        );

        state.open_plot_dialog(false);
        printers_of(&mut state);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.settle();
        assert_eq!(dialog.dpi, 600);
        assert_eq!(
            dialog.target,
            super::Target::Printer(crate::print_backend::PrinterId("Office".into()))
        );

        // A printer that is gone is not guessed at (the layout remembers
        // Office from that plot, so take the plot's saved setup back).
        state.undo();
        state.settings.plot_device = "Gone".into();
        state.open_plot_dialog(false);
        printers_of(&mut state);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.settle();
        assert_eq!(dialog.target, super::Target::Pdf);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_device_to_start_on_is_the_layouts_then_the_last_used() {
        use super::{initial_device, Target};
        assert_eq!(
            initial_device("OpenDraft SVG", "Office"),
            (Target::Svg, None)
        );
        assert_eq!(
            initial_device("none_device", "OpenDraft SVG"),
            (Target::Svg, None)
        );
        assert_eq!(initial_device("OpenDraft PDF/SVG", ""), (Target::Pdf, None));
        assert_eq!(
            initial_device("Lab", "OpenDraft SVG"),
            (Target::Pdf, Some("Lab".into())),
            "a printer is looked for once the printers are known"
        );
        assert_eq!(initial_device("", ""), (Target::Pdf, None));
    }

    #[test]
    fn a_layout_set_to_svg_opens_on_svg_and_plots_an_svg_file() {
        let (mut state, script, dir) = state();
        let mut layouts = state.session().document().layouts().to_vec();
        layouts[0].plot.device_name = "OpenDraft SVG".into();
        state.perform(opendraft_commands::SetLayouts::new("Page setup", layouts));
        state.open_plot_dialog(false);
        assert_eq!(
            state.command_dialogs.plot.as_ref().unwrap().target,
            super::Target::Svg
        );
        script.will_save_to(Some(dir.join("s")));
        assert!(state.plot_dialog_run());
        assert_eq!(script.filters(), ["SVG file"]);
        assert!(std::fs::read_to_string(dir.join("s.svg"))
            .unwrap()
            .contains("<svg"));
        assert_eq!(state.settings.plot_device, "OpenDraft SVG");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_model_tab_previews_plots_and_saves_its_own_setup() {
        let (mut state, script, dir) = state();
        state.enter_model();
        let layout_before = state.session().document().layouts()[0].plot.clone();
        state.open_plot_dialog(true);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        let preview = dialog.preview.clone().expect("a model preview");
        assert!(
            preview.rgba.iter().any(|b| *b != 255),
            "the line is on the page"
        );
        dialog.draft.plot.margins_mm = [10.0; 4];
        dialog.draft.plot.flags &= !opendraft_core::plot_settings::flags::PLOT_CENTERED;
        dialog.save_changes = true;
        script.will_save_to(Some(dir.join("m.pdf")));
        assert!(state.plot_dialog_run());
        let bytes = std::fs::read(dir.join("m.pdf")).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert_eq!(script.suggested().last().unwrap(), "Drawing1-Model.pdf");
        assert_eq!(
            state.session().document().model_plot().margins_mm,
            [10.0; 4]
        );
        assert_eq!(state.session().document().layouts()[0].plot, layout_before);
        state.undo();
        assert_eq!(state.session().document().model_plot().margins_mm, [0.0; 4]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_model_plot_of_an_empty_drawing_says_there_is_nothing_to_plot() {
        let mut state = AppState::new();
        state.open_plot_dialog(true);
        let dialog = state.command_dialogs.plot.as_ref().unwrap();
        assert!(dialog.preview.is_none());
        assert!(dialog.error.as_deref().unwrap().contains("nothing to plot"));
    }

    #[test]
    fn a_refused_paper_job_says_why_and_the_drawing_is_unchanged() {
        let (mut state, _, dir) = state();
        let fake = with_fake(&mut state);
        *fake.fail_with.lock().unwrap() = Some(crate::print_backend::PrintError::SpoolRejected(
            "queue full".into(),
        ));
        state.open_plot_dialog(false);
        printers_of(&mut state);
        state.command_dialogs.plot.as_mut().unwrap().target =
            super::Target::Printer(crate::print_backend::PrinterId("Office".into()));
        assert!(state.plot_dialog_run());
        assert!(state.message.contains("queue full"), "{}", state.message);
        assert!(fake.jobs.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sheet_the_printer_has_no_paper_for_is_refused_not_resized() {
        let (mut state, _, dir) = state();
        let fake = with_fake(&mut state);
        state.open_plot_dialog(false);
        printers_of(&mut state);
        let dialog = state.command_dialogs.plot.as_mut().unwrap();
        dialog.draft.width = 420.0;
        dialog.draft.height = 297.0;
        dialog.target = super::Target::Printer(crate::print_backend::PrinterId("Office".into()));
        assert!(state.plot_dialog_run());
        assert!(
            state.message.contains("no paper of that size"),
            "{}",
            state.message
        );
        assert!(fake.jobs.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_file_dialog_keeps_the_plot_dialog_open() {
        let (mut state, _, dir) = state();
        state.open_plot_dialog(false);
        assert!(!state.plot_dialog_run(), "the picker answers cancelled");
        assert!(state.command_dialogs.plot.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
