//! Paper output through CUPS: `lpstat` and `lpoptions` to ask, `lp` to print
//! a PDF. Every program gets its arguments as a list; the printer and paper
//! names come from CUPS's own listing and are checked before they are used.

use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::{
    resolve_media, safe_name, validate_job, Call, Capabilities, Medium, PrintBackend, PrintError,
    PrintJob, Printer, PrinterId, Receipt, Runner, TempDir, DISCOVERY_TIMEOUT, SPOOL_TIMEOUT,
};

pub struct CupsBackend<R: Runner> {
    runner: R,
}

impl<R: Runner> CupsBackend<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    fn ask(
        &self,
        program: &'static str,
        args: &[&str],
        cancel: &AtomicBool,
    ) -> Result<String, PrintError> {
        let call = Call {
            program,
            args: args.iter().map(OsString::from).collect(),
            envs: Vec::new(),
            timeout: DISCOVERY_TIMEOUT,
            kill: true,
        };
        let output = self.runner.run(&call, cancel)?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(PrintError::Capability(first_line(&output.stderr)))
        }
    }
}

impl<R: Runner> PrintBackend for CupsBackend<R> {
    fn printers(&self) -> Result<Vec<Printer>, PrintError> {
        let never = AtomicBool::new(false);
        let names = self.ask("lpstat", &["-e"], &never)?;
        // No default is not an error: lpstat says so on stderr and fails.
        let default = self
            .ask("lpstat", &["-d"], &never)
            .ok()
            .and_then(|text| parse_default(&text));
        Ok(parse_printers(&names, default.as_deref()))
    }

    fn capabilities(&self, printer: &PrinterId) -> Result<Capabilities, PrintError> {
        if !safe_name(&printer.0) {
            return Err(PrintError::Invalid("the printer name cannot be used"));
        }
        let text = self.ask(
            "lpoptions",
            &["-p", &printer.0, "-l"],
            &AtomicBool::new(false),
        )?;
        Ok(Capabilities {
            media: parse_media(&text),
        })
    }

    fn submit(&self, job: &PrintJob, cancel: &AtomicBool) -> Result<Receipt, PrintError> {
        validate_job(job)?;
        let capabilities = self.capabilities(&job.printer)?;
        let medium = resolve_media(&capabilities, &job.sheet)?;
        let dir = TempDir::new()?;
        let file = dir.write("plot.pdf", &od_files::pdf(&job.sheet))?;
        let call = Call {
            program: "lp",
            args: lp_args(job, &medium.name, &file),
            envs: Vec::new(),
            timeout: SPOOL_TIMEOUT,
            kill: true,
        };
        let output = self.runner.run(&call, cancel)?;
        if output.success {
            Ok(Receipt {
                queue_id: parse_job_id(&output.stdout),
            })
        } else {
            Err(PrintError::SpoolRejected(first_line(&output.stderr)))
        }
    }
}

/// The arguments of `lp` for `job` on paper `media`, printing `file` at its
/// own size. The file follows `--`, so no name can pass for an option.
pub fn lp_args(job: &PrintJob, media: &str, file: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "-d".into(),
        job.printer.0.clone().into(),
        "-n".into(),
        job.copies.to_string().into(),
        "-t".into(),
        "OpenDraft plot".into(),
        "-o".into(),
        format!("media={media}").into(),
        "-o".into(),
        "scaling=100".into(),
        "--".into(),
    ];
    args.push(file.as_os_str().to_owned());
    args
}

/// The printers `lpstat -e` lists, one per line; a name that could pass for
/// an option or holds control characters is left out.
pub fn parse_printers(text: &str, default: Option<&str>) -> Vec<Printer> {
    text.lines()
        .map(str::trim)
        .filter(|name| safe_name(name))
        .map(|name| Printer {
            id: PrinterId(name.to_owned()),
            is_default: default == Some(name),
        })
        .collect()
}

/// The name in "system default destination: Office".
pub fn parse_default(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.split_once("destination:"))
        .map(|(_, name)| name.trim().to_owned())
        .filter(|name| safe_name(name))
}

/// The id in "request id is Office-12 (1 file(s))".
pub fn parse_job_id(text: &str) -> Option<String> {
    let rest = text.split("request id is").nth(1)?;
    let id = rest.split_whitespace().next()?;
    safe_name(id).then(|| id.to_owned())
}

/// The sizes `lpoptions -l` offers under PageSize (or media), with their
/// sizes where the name says them.
pub fn parse_media(text: &str) -> Vec<Medium> {
    let Some(line) = text
        .lines()
        .find(|l| l.starts_with("PageSize/") || l.starts_with("media/"))
    else {
        return Vec::new();
    };
    let options = line.split_once(':').map_or("", |(_, options)| options);
    options
        .split_whitespace()
        .map(|name| name.trim_start_matches('*'))
        .filter(|name| safe_name(name))
        .filter_map(|name| {
            let (width_mm, height_mm) = media_size(name)?;
            Some(Medium {
                name: name.to_owned(),
                width_mm,
                height_mm,
            })
        })
        .collect()
}

/// A paper's size in millimetres from its CUPS name: a PostScript name
/// (`A4`, `Letter`) or a PWG one (`iso_a4_210x297mm`, `na_letter_8.5x11in`).
pub fn media_size(name: &str) -> Option<(f64, f64)> {
    const NAMED: &[(&str, f64, f64)] = &[
        ("A0", 841.0, 1189.0),
        ("A1", 594.0, 841.0),
        ("A2", 420.0, 594.0),
        ("A3", 297.0, 420.0),
        ("A4", 210.0, 297.0),
        ("A5", 148.0, 210.0),
        ("Letter", 215.9, 279.4),
        ("Legal", 215.9, 355.6),
        ("Tabloid", 279.4, 431.8),
        ("11x17", 279.4, 431.8),
        ("Ledger", 431.8, 279.4),
    ];
    if let Some((_, w, h)) = NAMED.iter().find(|(n, ..)| *n == name) {
        return Some((*w, *h));
    }
    let size = name.rsplit('_').next()?;
    let (unit, scale) = if let Some(s) = size.strip_suffix("mm") {
        (s, 1.0)
    } else {
        (size.strip_suffix("in")?, 25.4)
    };
    let (w, h) = unit.split_once('x')?;
    let (w, h) = (w.parse::<f64>().ok()?, h.parse::<f64>().ok()?);
    (w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()).then_some((w * scale, h * scale))
}

fn first_line(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no reason given")
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use od_core::plot::PlotSheet;

    use super::super::{Output, PrintError};
    use super::*;

    fn sheet() -> PlotSheet {
        PlotSheet {
            width: 297.0,
            height: 210.0,
            paths: Vec::new(),
            fills: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn job(name: &str) -> PrintJob {
        PrintJob {
            printer: PrinterId(name.into()),
            copies: 2,
            dpi: 300,
            sheet: sheet(),
        }
    }

    /// Answers each program from a list and remembers what it was asked.
    struct Script {
        answers: Mutex<Vec<(&'static str, Result<Output, PrintError>)>>,
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl Script {
        fn new(answers: Vec<(&'static str, Result<Output, PrintError>)>) -> Self {
            Self {
                answers: Mutex::new(answers),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl Runner for &Script {
        fn run(&self, call: &Call, _: &AtomicBool) -> Result<Output, PrintError> {
            self.calls.lock().unwrap().push((
                call.program.to_owned(),
                call.args
                    .iter()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect(),
            ));
            let mut answers = self.answers.lock().unwrap();
            let at = answers.iter().position(|(p, _)| *p == call.program);
            at.map(|i| answers.remove(i).1)
                .unwrap_or(Err(PrintError::BackendUnavailable("not scripted")))
        }
    }

    fn ok(stdout: &str) -> Result<Output, PrintError> {
        Ok(Output {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
        })
    }

    const OPTIONS: &str =
        "PageSize/Media Size: Letter *A4 A3 Custom.WIDTHxHEIGHT\nResolution/Resolution: *600dpi\n";

    #[test]
    fn printers_and_the_default_are_read_and_odd_names_dropped() {
        let printers = parse_printers(
            "Office\n-evil\n\nLab Printer\nbad\u{7}\n",
            Some("Lab Printer"),
        );
        let names: Vec<_> = printers.iter().map(|p| p.id.0.as_str()).collect();
        assert_eq!(names, ["Office", "Lab Printer"]);
        assert!(printers[1].is_default && !printers[0].is_default);
        assert_eq!(
            parse_default("system default destination: Office\n"),
            Some("Office".into())
        );
        assert_eq!(parse_default("no system default destination\n"), None);
    }

    #[test]
    fn media_names_become_sizes_and_custom_is_skipped() {
        let media = parse_media(OPTIONS);
        let names: Vec<_> = media.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Letter", "A4", "A3"]);
        assert_eq!((media[1].width_mm, media[1].height_mm), (210.0, 297.0));
        assert_eq!(media_size("iso_a4_210x297mm"), Some((210.0, 297.0)));
        let (w, h) = media_size("na_letter_8.5x11in").unwrap();
        assert!((w - 215.9).abs() < 1e-9 && (h - 279.4).abs() < 1e-9);
        assert_eq!(media_size("Mystery"), None);
        assert!(parse_media("Resolution/Resolution: *600dpi\n").is_empty());
    }

    #[test]
    fn the_job_id_is_read_from_lps_answer() {
        assert_eq!(
            parse_job_id("request id is Office-12 (1 file(s))\n"),
            Some("Office-12".into())
        );
        assert_eq!(parse_job_id("something else"), None);
    }

    #[test]
    fn lp_gets_its_arguments_as_a_list_and_the_file_after_the_separator() {
        let args: Vec<String> = lp_args(
            &job("My Printer; rm -rf /"),
            "A4",
            Path::new("/tmp/x/plot.pdf"),
        )
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
        assert_eq!(
            args,
            [
                "-d",
                "My Printer; rm -rf /",
                "-n",
                "2",
                "-t",
                "OpenDraft plot",
                "-o",
                "media=A4",
                "-o",
                "scaling=100",
                "--",
                "/tmp/x/plot.pdf"
            ]
        );
    }

    #[test]
    fn a_job_is_sent_on_the_papers_own_name_and_its_id_returned() {
        let script = Script::new(vec![
            ("lpoptions", ok(OPTIONS)),
            ("lp", ok("request id is Office-7 (1 file(s))\n")),
        ]);
        let backend = CupsBackend::new(&script);
        let receipt = backend
            .submit(&job("Office"), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(receipt.queue_id.as_deref(), Some("Office-7"));
        let calls = script.calls.lock().unwrap();
        assert_eq!(calls[0].0, "lpoptions");
        assert!(calls[1].1.contains(&"media=A4".to_owned()));
    }

    #[test]
    fn a_missing_paper_a_refusal_and_a_missing_cups_are_told_apart() {
        let mut tall = job("Office");
        tall.sheet = PlotSheet {
            width: 1189.0,
            height: 841.0,
            ..sheet()
        };
        let script = Script::new(vec![("lpoptions", ok(OPTIONS))]);
        assert_eq!(
            CupsBackend::new(&script).submit(&tall, &AtomicBool::new(false)),
            Err(PrintError::MissingMedia)
        );
        let script = Script::new(vec![
            ("lpoptions", ok(OPTIONS)),
            (
                "lp",
                Ok(Output {
                    success: false,
                    stdout: String::new(),
                    stderr: "lp: The printer or class does not exist.\n".into(),
                }),
            ),
        ]);
        assert_eq!(
            CupsBackend::new(&script).submit(&job("Office"), &AtomicBool::new(false)),
            Err(PrintError::SpoolRejected(
                "lp: The printer or class does not exist.".into()
            ))
        );
        let script = Script::new(Vec::new());
        assert!(matches!(
            CupsBackend::new(&script).submit(&job("Office"), &AtomicBool::new(false)),
            Err(PrintError::BackendUnavailable(_))
        ));
    }

    #[test]
    fn a_hostile_printer_name_never_reaches_a_program() {
        let script = Script::new(Vec::new());
        let backend = CupsBackend::new(&script);
        assert!(backend
            .submit(&job("-o evil"), &AtomicBool::new(false))
            .is_err());
        assert!(backend.capabilities(&PrinterId("-x".into())).is_err());
        assert!(script.calls.lock().unwrap().is_empty());
    }
}
