//! Paper output on Windows through a fixed PowerShell script over
//! `System.Drawing.Printing` (no printer-driver code of our own, and no
//! `unsafe`). The script is a constant; what to do is an environment variable
//! and the job's data a JSON file, so a printer called `"; Remove-Item ...` is
//! only ever a string. The sheet is printed as a picture at the job's
//! resolution.

use std::ffi::OsString;
use std::sync::atomic::AtomicBool;

use base64::Engine as _;
use serde_json::{json, Value};

use super::{
    raster_size, resolve_media, safe_name, validate_job, Call, Capabilities, Medium, PrintBackend,
    PrintError, PrintJob, Printer, PrinterId, Receipt, Runner, TempDir, DISCOVERY_TIMEOUT,
    SPOOL_TIMEOUT,
};

const SCRIPT: &str = include_str!("print_windows.ps1");

pub struct WindowsBackend<R: Runner> {
    runner: R,
}

impl<R: Runner> WindowsBackend<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    fn call(&self, mode: &'static str, job_file: Option<&std::path::Path>, spool: bool) -> Call {
        let mut envs: Vec<(&'static str, OsString)> = vec![("OPENDRAFT_PRINT_MODE", mode.into())];
        if let Some(file) = job_file {
            envs.push(("OPENDRAFT_PRINT_JOB", file.as_os_str().to_owned()));
        }
        Call {
            program: "powershell.exe",
            args: script_args(),
            envs,
            timeout: if spool {
                SPOOL_TIMEOUT
            } else {
                DISCOVERY_TIMEOUT
            },
            // A job the spooler has accepted must not be cut off half way.
            kill: !spool,
        }
    }

    fn run(&self, call: &Call, cancel: &AtomicBool) -> Result<String, PrintError> {
        let output = self.runner.run(call, cancel)?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(PrintError::Capability(first_line(&output.stderr)))
        }
    }
}

impl<R: Runner> PrintBackend for WindowsBackend<R> {
    fn printers(&self) -> Result<Vec<Printer>, PrintError> {
        let text = self.run(&self.call("list", None, false), &AtomicBool::new(false))?;
        parse_printers(&text)
    }

    fn capabilities(&self, printer: &PrinterId) -> Result<Capabilities, PrintError> {
        if !safe_name(&printer.0) {
            return Err(PrintError::Invalid("the printer name cannot be used"));
        }
        let dir = TempDir::new()?;
        let file = dir.write(
            "caps.json",
            json!({ "printer": printer.0 }).to_string().as_bytes(),
        )?;
        let text = self.run(
            &self.call("caps", Some(&file), false),
            &AtomicBool::new(false),
        )?;
        parse_media(&text)
    }

    fn submit(&self, job: &PrintJob, cancel: &AtomicBool) -> Result<Receipt, PrintError> {
        validate_job(job)?;
        let capabilities = self.capabilities(&job.printer)?;
        let medium = resolve_media(&capabilities, &job.sheet)?;
        let (width, height) = raster_size(job.sheet.width, job.sheet.height, job.dpi)?;
        let picture = opendraft_files::png(&job.sheet, width, height)
            .map_err(|e| PrintError::TemporaryFile(e.to_string()))?;
        let dir = TempDir::new()?;
        let png = dir.write("plot.png", &picture)?;
        let data = job_json(job, &medium.name, &png);
        let file = dir.write("job.json", data.to_string().as_bytes())?;
        self.run(&self.call("print", Some(&file), true), cancel)
            .map_err(|e| match e {
                PrintError::Capability(why) => PrintError::SpoolRejected(why),
                other => other,
            })?;
        Ok(Receipt::default())
    }
}

/// The arguments that run [`SCRIPT`] without a file or a profile.
fn script_args() -> Vec<OsString> {
    let utf16: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    vec![
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-EncodedCommand".into(),
        base64::engine::general_purpose::STANDARD
            .encode(utf16)
            .into(),
    ]
}

/// The data of a print job for the script. The sheet lies on the paper in
/// the orientation it was cut: a wider-than-tall sheet is landscape.
pub fn job_json(job: &PrintJob, media: &str, png: &std::path::Path) -> Value {
    json!({
        "printer": job.printer.0,
        "copies": job.copies,
        "media": media,
        "png": png.to_string_lossy(),
        "width_mm": job.sheet.width,
        "height_mm": job.sheet.height,
        "landscape": job.sheet.width > job.sheet.height,
    })
}

fn items(text: &str) -> Result<Vec<Value>, PrintError> {
    match serde_json::from_str::<Value>(text.trim()) {
        Ok(Value::Array(list)) => Ok(list),
        Ok(object @ Value::Object(_)) => Ok(vec![object]),
        _ => Err(PrintError::Capability(
            "the answer was not understood".into(),
        )),
    }
}

pub fn parse_printers(text: &str) -> Result<Vec<Printer>, PrintError> {
    Ok(items(text)?
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?;
            safe_name(name).then(|| Printer {
                id: PrinterId(name.to_owned()),
                is_default: item
                    .get("default")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect())
}

pub fn parse_media(text: &str) -> Result<Capabilities, PrintError> {
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|_| PrintError::Capability("the answer was not understood".into()))?;
    let media = value
        .get("media")
        .and_then(|m| match m {
            Value::Array(list) => Some(list.clone()),
            Value::Object(_) => Some(vec![m.clone()]),
            _ => None,
        })
        .unwrap_or_default();
    Ok(Capabilities {
        media: media
            .iter()
            .filter_map(|item| {
                let name = item.get("name")?.as_str()?;
                let (width, height) = (item.get("width")?.as_f64()?, item.get("height")?.as_f64()?);
                (safe_name(name) && width > 0.0 && height > 0.0).then(|| Medium {
                    name: name.to_owned(),
                    width_mm: width,
                    height_mm: height,
                })
            })
            .collect(),
    })
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
    use std::path::Path;
    use std::sync::Mutex;

    use opendraft_core::plot::PlotSheet;

    use super::super::Output;
    use super::*;

    fn job(name: &str) -> PrintJob {
        PrintJob {
            printer: PrinterId(name.into()),
            copies: 3,
            dpi: 100,
            sheet: PlotSheet {
                width: 297.0,
                height: 210.0,
                paths: Vec::new(),
                fills: Vec::new(),
                notes: Vec::new(),
            },
        }
    }

    #[test]
    fn a_hostile_printer_name_is_data_in_the_job_file_not_part_of_any_command() {
        let name = "x\"; Remove-Item C:\\ -Recurse; \"";
        let data = job_json(
            &job(name),
            "A4 (210 x 297 mm)",
            Path::new("C:\\t\\plot.png"),
        );
        assert_eq!(data["printer"], name);
        assert_eq!(data["copies"], 3);
        assert_eq!(data["landscape"], true);
        assert!(!SCRIPT.contains(name));
        let args = script_args();
        assert!(args
            .iter()
            .all(|a| !a.to_string_lossy().contains("Remove-Item")));
        assert_eq!(args[4], "-EncodedCommand");
    }

    #[test]
    fn the_script_decodes_back_to_the_fixed_text() {
        let args = script_args();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(args[5].to_string_lossy().as_bytes())
            .unwrap();
        let units: Vec<u16> = bytes
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), SCRIPT);
    }

    #[test]
    fn printers_and_papers_are_read_from_the_scripts_json() {
        let printers = parse_printers(
            r#"[{"name":"Office","default":true},{"name":"-evil","default":false},{"name":"Lab"}]"#,
        )
        .unwrap();
        let names: Vec<_> = printers.iter().map(|p| p.id.0.as_str()).collect();
        assert_eq!(names, ["Office", "Lab"]);
        assert!(printers[0].is_default && !printers[1].is_default);
        assert_eq!(
            parse_printers(r#"{"name":"Solo","default":true}"#)
                .unwrap()
                .len(),
            1
        );
        assert!(parse_printers("not json").is_err());

        let caps = parse_media(
            r#"{"media":[{"name":"A4","width":210.0,"height":297.0},{"name":"Bad","width":0,"height":5}]}"#,
        )
        .unwrap();
        assert_eq!(caps.media.len(), 1);
        assert_eq!(caps.media[0].name, "A4");
        assert!(parse_media(r#"{"media":[]}"#).unwrap().media.is_empty());
    }

    struct Fixed(Mutex<Vec<(String, Option<OsString>)>>);

    impl Runner for &Fixed {
        fn run(&self, call: &Call, _: &AtomicBool) -> Result<Output, PrintError> {
            let mode = call
                .envs
                .iter()
                .find(|(k, _)| *k == "OPENDRAFT_PRINT_MODE")
                .unwrap()
                .1
                .clone();
            let mode = mode.to_string_lossy().into_owned();
            let file = call
                .envs
                .iter()
                .find(|(k, _)| *k == "OPENDRAFT_PRINT_JOB")
                .map(|(_, v)| v.clone());
            // The job file must be there while the script runs.
            if let Some(f) = &file {
                assert!(Path::new(f).exists());
            }
            self.0.lock().unwrap().push((mode.clone(), file));
            let stdout = match mode.as_str() {
                "caps" => r#"{"media":[{"name":"A4","width":210,"height":297}]}"#,
                "print" => "ok",
                _ => r#"[{"name":"Office","default":true}]"#,
            };
            Ok(Output {
                success: true,
                stdout: stdout.into(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn a_job_makes_a_picture_and_a_job_file_and_cleans_up() {
        let runner = Fixed(Mutex::new(Vec::new()));
        let backend = WindowsBackend::new(&runner);
        assert!(backend
            .submit(&job("Office"), &AtomicBool::new(false))
            .is_ok());
        let calls = runner.0.lock().unwrap();
        let modes: Vec<_> = calls.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(modes, ["caps", "print"]);
        let file = calls[1].1.clone().unwrap();
        assert!(!Path::new(&file).exists(), "the temporary folder is gone");
    }

    #[test]
    fn a_page_over_the_pixel_cap_is_refused_before_a_picture_is_made() {
        let runner = Fixed(Mutex::new(Vec::new()));
        let backend = WindowsBackend::new(&runner);
        let mut big = job("Office");
        big.dpi = 1200;
        let err = backend.submit(&big, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(err, PrintError::RasterTooLarge);
        let modes = runner
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| m == "print")
            .count();
        assert_eq!(modes, 0, "nothing was sent to the spooler");
    }
}
