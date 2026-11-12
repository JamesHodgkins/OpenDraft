//! -PLOT: the command-line form of PLOT. It asks for a layout, an output
//! device and (when detailed configuration is wanted) the page setup, then
//! hands one request to the app, which writes the file or sends the job
//! exactly as the Plot dialog does. Typing "?" at a name lists what there is.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use opendraft_commands::{Answer, Cancelled, Editor, Options};
use opendraft_core::plot_input::{parse_offset, parse_scale, scale_text};
use opendraft_core::{
    plot_settings::{flags, DEVICE_PDF, DEVICE_SVG},
    PaperUnits, PlotArea, PlotRotation, PlotSettings, PAPER_SIZES,
};

use opendraft_core::plot::PlotSpace;

use crate::layouts::PageDraft;
use crate::plot_state::PlotFormat;
use crate::print_backend::{PrintBackend, Printer, PrinterId};

/// Where the plot goes.
pub enum Destination {
    File(PathBuf),
    Printer {
        printer: PrinterId,
        copies: u32,
        dpi: u32,
    },
}

/// A finished -PLOT, for the app to carry out once the command has ended.
pub struct PlotRequest {
    pub draft: PageDraft,
    pub destination: Destination,
    pub save_changes: bool,
}

pub type PlotRequests = Arc<Mutex<Vec<PlotRequest>>>;

/// What the command needs from the app, taken before it starts.
pub struct Setup {
    pub requests: PlotRequests,
    pub backend: Arc<dyn PrintBackend>,
    /// Where a file plot is suggested, and the drawing's name.
    pub folder: Option<PathBuf>,
    pub title: String,
    pub last_device: String,
    pub dpi: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device {
    Pdf,
    Svg,
    Printer(PrinterId),
}

impl Device {
    /// The device as a page setup names it.
    fn name(&self) -> String {
        match self {
            Self::Pdf => DEVICE_PDF.to_owned(),
            Self::Svg => DEVICE_SVG.to_owned(),
            Self::Printer(printer) => printer.0.clone(),
        }
    }
}

/// The device a typed name means: OpenDraft's PDF or SVG (`PDF`, `OD SVG`,
/// `OpenDraft PDF`...), a printer by its name (any case), or a printer whose
/// name starts with it when only one does.
pub fn resolve_device(text: &str, printers: &[Printer]) -> Option<Device> {
    let text = text.trim();
    let is = |names: &[&str]| names.iter().any(|n| text.eq_ignore_ascii_case(n));
    if is(&["pdf", "file", "od pdf", DEVICE_PDF]) {
        return Some(Device::Pdf);
    }
    if is(&["svg", "od svg", DEVICE_SVG]) {
        return Some(Device::Svg);
    }
    let lower = text.to_lowercase();
    if let Some(p) = printers.iter().find(|p| p.id.0.to_lowercase() == lower) {
        return Some(Device::Printer(p.id.clone()));
    }
    let mut starts = printers
        .iter()
        .filter(|p| !lower.is_empty() && p.id.0.to_lowercase().starts_with(&lower));
    match (starts.next(), starts.next()) {
        (Some(p), None) => Some(Device::Printer(p.id.clone())),
        _ => None,
    }
}

/// A paper by its name (`A3`) or as `width,height` in millimetres, shown
/// in the orientation `landscape` asks for.
pub fn parse_paper(text: &str, landscape: bool) -> Option<(String, f64, f64)> {
    let text = text.trim();
    let (name, long, short) = match PAPER_SIZES
        .iter()
        .find(|(n, ..)| n.eq_ignore_ascii_case(text))
    {
        Some((n, w, h)) => ((*n).to_owned(), w.max(*h), w.min(*h)),
        None => {
            let point = parse_offset(&text.replace(['x', 'X'], ","))?;
            let side = |v: f64| (10.0..=10_000.0).contains(&v);
            if !(side(point.x) && side(point.y)) {
                return None;
            }
            (
                "Custom".to_owned(),
                point.x.max(point.y),
                point.x.min(point.y),
            )
        }
    };
    Some(if landscape {
        (name, long, short)
    } else {
        (name, short, long)
    })
}

fn say_names(ed: &Editor, what: &str, names: &[String]) {
    ed.say(format!("{what}: {}", names.join(", ")));
}

fn yes_no(ed: &Editor, message: &str, default: bool) -> Result<bool, Cancelled> {
    let options = Options::<String>::new(format!("{message} [Yes/No]"))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default(if default { "Yes" } else { "No" }.to_owned());
    Ok(ed.ask_keyword(options)?.as_deref() == Some("Yes"))
}

/// A line of text, its default taken on a bare Enter.
fn text(ed: &Editor, message: &str, default: &str) -> Result<String, Cancelled> {
    let options = Options::<String>::new(message).default(default.to_owned());
    Ok(match ed.ask_string(options)? {
        Answer::Value(t) | Answer::Keyword(t) => t,
        Answer::Enter => default.to_owned(),
    })
}

fn set_flag(plot: &mut PlotSettings, flag: u16, on: bool) {
    if on {
        plot.flags |= flag;
    } else {
        plot.flags &= !flag;
    }
}

fn set_upside_down(plot: &mut PlotSettings, upside: bool) {
    plot.rotation = match (plot.rotation, upside) {
        (PlotRotation::Degrees0 | PlotRotation::Degrees180, true) => PlotRotation::Degrees180,
        (PlotRotation::Degrees0 | PlotRotation::Degrees180, false) => PlotRotation::Degrees0,
        (_, true) => PlotRotation::Clockwise90,
        (_, false) => PlotRotation::CounterClockwise90,
    };
}

/// The page setup questions of a detailed -PLOT, applied to `draft`.
fn detailed(ed: &Editor, draft: &mut PageDraft) -> Result<(), Cancelled> {
    loop {
        let landscape = draft.width >= draft.height;
        let current = draft.plot.canonical_media_name.clone();
        let answer = text(ed, "Enter paper size or [?]", &current)?;
        if answer == "?" {
            let names: Vec<String> = PAPER_SIZES.iter().map(|(n, ..)| (*n).to_owned()).collect();
            say_names(ed, "Paper sizes (or width,height in mm)", &names);
        } else if let Some((name, w, h)) = parse_paper(&answer, landscape) {
            (draft.width, draft.height) = (w, h);
            draft.plot.canonical_media_name = name;
            break;
        } else {
            ed.say(format!("\"{answer}\" is not a paper size."));
        }
    }
    let units = Options::<String>::new("Enter paper units [Inches/Millimeters]")
        .keyword("Inches", "I")
        .keyword("Millimeters", "M")
        .default("Millimeters".to_owned());
    draft.plot.paper_units = match ed.ask_keyword(units)?.as_deref() {
        Some("Inches") => PaperUnits::Inches,
        _ => PaperUnits::Millimeters,
    };
    let quarter = matches!(
        draft.plot.rotation,
        PlotRotation::CounterClockwise90 | PlotRotation::Clockwise90
    );
    let now = if (draft.width >= draft.height) != quarter {
        "Landscape"
    } else {
        "Portrait"
    };
    let orientation = Options::<String>::new("Enter drawing orientation [Portrait/Landscape]")
        .keyword("Portrait", "P")
        .keyword("Landscape", "L")
        .default(now.to_owned());
    if ed.ask_keyword(orientation)?.as_deref().unwrap_or(now) != now {
        (draft.width, draft.height) = (draft.height, draft.width);
    }
    let upside = matches!(
        draft.plot.rotation,
        PlotRotation::Degrees180 | PlotRotation::Clockwise90
    );
    set_upside_down(&mut draft.plot, yes_no(ed, "Plot upside down?", upside)?);
    area(ed, draft)?;
    scale_and_offset(ed, draft)?;
    ed.say("Plot styles are not applied yet: the layout's style table and plot-style options are left as they are.");
    let plot = &mut draft.plot;
    let lineweights = yes_no(
        ed,
        "Plot with lineweights?",
        plot.flags & flags::PRINT_LINEWEIGHTS != 0,
    )?;
    set_flag(plot, flags::PRINT_LINEWEIGHTS, lineweights);
    let scaled = yes_no(
        ed,
        "Scale lineweights with plot scale?",
        plot.flags & flags::SCALE_LINEWEIGHTS != 0,
    )?;
    set_flag(plot, flags::SCALE_LINEWEIGHTS, scaled);
    let last = yes_no(
        ed,
        "Plot paper space last?",
        plot.flags & flags::DRAW_VIEWPORTS_FIRST != 0,
    )?;
    set_flag(plot, flags::DRAW_VIEWPORTS_FIRST, last);
    let borders = yes_no(
        ed,
        "Plot viewport borders?",
        plot.flags & flags::PLOT_VIEWPORT_BORDERS != 0,
    )?;
    set_flag(plot, flags::PLOT_VIEWPORT_BORDERS, borders);
    Ok(())
}

fn area(ed: &Editor, draft: &mut PageDraft) -> Result<(), Cancelled> {
    loop {
        let options =
            Options::<String>::new("Enter plot area [Display/Extents/Limits/View/Window/Layout]")
                .keyword("Display", "D")
                .keyword("Extents", "E")
                .keyword("Limits", "L")
                .keyword("View", "V")
                .keyword("Window", "W")
                .keyword("Layout", "La")
                .default("Layout".to_owned());
        match ed.ask_keyword(options)?.as_deref().unwrap_or("Layout") {
            "Display" | "View" => {
                ed.say("That plot area is not available: use Extents, Limits, Window or Layout.");
                continue;
            }
            "Extents" => draft.plot.area = PlotArea::Extents,
            "Limits" => draft.plot.area = PlotArea::Limits,
            "Window" => {
                let (first, second) = crate::plot_pick::ask_window(ed)?;
                draft.plot.area = PlotArea::Window { first, second };
            }
            _ => draft.plot.area = PlotArea::Layout,
        }
        return Ok(());
    }
}

fn scale_and_offset(ed: &Editor, draft: &mut PageDraft) -> Result<(), Cancelled> {
    loop {
        let shown = scale_text(&draft.plot.scale);
        let answer = text(
            ed,
            "Enter plot scale (plotted millimeters=drawing units) or [Fit]",
            &shown,
        )?;
        match parse_scale(&answer) {
            Some(scale) => {
                let fit = matches!(
                    scale,
                    opendraft_core::PlotScale::Standard {
                        kind: opendraft_core::StandardScale::Fit,
                        ..
                    }
                );
                set_flag(&mut draft.plot, flags::USE_STANDARD_SCALE, fit);
                draft.plot.scale = scale;
                break;
            }
            None => ed.say(format!(
                "\"{answer}\" is not a scale; try 1:50, 2:1 or Fit."
            )),
        }
    }
    loop {
        let shown = format!("{},{}", draft.plot.origin_mm.x, draft.plot.origin_mm.y);
        let answer = text(ed, "Enter plot offset (x,y) or [Center]", &shown)?;
        if answer.eq_ignore_ascii_case("center") || answer.eq_ignore_ascii_case("c") {
            set_flag(&mut draft.plot, flags::PLOT_CENTERED, true);
            return Ok(());
        }
        match parse_offset(&answer) {
            Some(offset) => {
                set_flag(&mut draft.plot, flags::PLOT_CENTERED, false);
                draft.plot.origin_mm = offset;
                return Ok(());
            }
            None => ed.say(format!("\"{answer}\" is not an offset; type x,y.")),
        }
    }
}

/// -PLOT.
pub fn plot_prompt(ed: &mut Editor, setup: Setup) -> Result<(), Cancelled> {
    // The Model tab and the layouts, by name; the one being worked in first.
    let (names, active) = ed.read(|d| {
        let mut names = vec!["Model".to_owned()];
        names.extend(d.layouts().iter().map(|l| l.name.clone()));
        let active = d
            .active_layout()
            .and_then(|id| d.layout(id))
            .map_or_else(|| "Model".to_owned(), |l| l.name.clone());
        (names, active)
    })?;
    let detail = yes_no(ed, "Detailed plot configuration?", false)?;
    let mut draft = None;
    while draft.is_none() {
        let answer = text(ed, "Enter a layout name or [?]", &active)?;
        if answer == "?" {
            say_names(ed, "Layouts", &names);
            continue;
        }
        let wanted = answer.trim().to_owned();
        draft = ed.read(move |d| {
            let space = if wanted.eq_ignore_ascii_case("model") {
                PlotSpace::Model
            } else {
                PlotSpace::Layout(
                    d.layouts()
                        .iter()
                        .find(|l| l.name.eq_ignore_ascii_case(&wanted))?
                        .id,
                )
            };
            PageDraft::of(d, space)
        })?;
        if draft.is_none() {
            ed.say(format!("There is no layout called \"{answer}\"."));
        }
    }
    let Some(mut draft) = draft else {
        return Ok(());
    };
    let printers = setup.backend.printers().unwrap_or_default();
    let fallback = resolve_device(&setup.last_device, &printers)
        .unwrap_or(Device::Pdf)
        .name();
    let device = loop {
        let answer = text(ed, "Enter an output device name or [?]", &fallback)?;
        if answer == "?" {
            let mut names = vec![DEVICE_PDF.to_owned(), DEVICE_SVG.to_owned()];
            names.extend(printers.iter().map(|p| p.id.0.clone()));
            say_names(ed, "Output devices", &names);
        } else if let Some(device) = resolve_device(&answer, &printers) {
            break device;
        } else {
            ed.say(format!("There is no output device called \"{answer}\"."));
        }
    };
    draft.plot.device_name = device.name();
    if detail {
        detailed(ed, &mut draft)?;
    }
    draft.plot.paper_width_mm = draft.width;
    draft.plot.paper_height_mm = draft.height;
    let destination = match device.clone() {
        Device::Pdf | Device::Svg => {
            let format = if device == Device::Svg {
                PlotFormat::Svg
            } else {
                PlotFormat::Pdf
            };
            let mut path = setup.folder.clone().unwrap_or_default();
            path.push(format!(
                "{}-{}.{}",
                setup.title,
                draft.name,
                format.extension()
            ));
            let answer = text(ed, "Enter file name", &path.to_string_lossy())?;
            Destination::File(format.named(PathBuf::from(answer.trim())))
        }
        Device::Printer(printer) => {
            let copies =
                ed.ask_number(Options::<f64>::new("Enter number of copies").default(1.0))?;
            let copies = match copies {
                Answer::Value(n) if n >= 1.0 => n.round().min(999.0) as u32,
                _ => 1,
            };
            Destination::Printer {
                printer,
                copies,
                dpi: setup.dpi,
            }
        }
    };
    let save_changes = yes_no(ed, "Save changes to layout?", false)?;
    if !yes_no(ed, "Proceed with plot?", true)? {
        return Ok(());
    }
    if let Ok(mut list) = setup.requests.lock() {
        list.push(PlotRequest {
            draft,
            destination,
            save_changes,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printers() -> Vec<Printer> {
        ["Office Laser", "Office Plotter", "Lab"]
            .iter()
            .map(|n| Printer {
                id: PrinterId((*n).into()),
                is_default: false,
            })
            .collect()
    }

    #[test]
    fn a_device_is_found_by_name_in_any_case_or_by_a_unique_start() {
        let list = printers();
        assert_eq!(resolve_device("pdf", &list), Some(Device::Pdf));
        assert_eq!(resolve_device("OD PDF", &list), Some(Device::Pdf));
        assert_eq!(resolve_device("opendraft svg", &list), Some(Device::Svg));
        assert_eq!(resolve_device("svg", &list), Some(Device::Svg));
        assert_eq!(
            resolve_device("lab", &list),
            Some(Device::Printer(PrinterId("Lab".into())))
        );
        assert_eq!(
            resolve_device("office l", &list),
            Some(Device::Printer(PrinterId("Office Laser".into())))
        );
        assert_eq!(resolve_device("office", &list), None, "two start that way");
        assert_eq!(resolve_device("", &list), None);
        assert_eq!(resolve_device("nothing", &list), None);
    }

    #[test]
    fn a_paper_is_a_name_or_a_size_in_the_orientation_asked_for() {
        assert_eq!(parse_paper("a3", true), Some(("A3".into(), 420.0, 297.0)));
        assert_eq!(parse_paper("A3", false), Some(("A3".into(), 297.0, 420.0)));
        assert_eq!(
            parse_paper("500x300", true),
            Some(("Custom".into(), 500.0, 300.0))
        );
        assert_eq!(
            parse_paper("300,500", true),
            Some(("Custom".into(), 500.0, 300.0))
        );
        for bad in ["", "Z9", "5x5", "20000,300", "1,2,3"] {
            assert_eq!(parse_paper(bad, true), None, "{bad}");
        }
    }
}
