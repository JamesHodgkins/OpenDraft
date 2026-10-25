//! A record of crashes. A Windows release build has no console window, so a
//! panic would leave no trace at all; this writes each one (when, the version,
//! the thread, the message, where it happened and a backtrace) to `crash.log`
//! in the profile's folder (see `profile::app_data`), and then lets the usual
//! hook run, so a terminal still shows it. A panic inside a command is caught
//! and the drawing carries on, but it is logged here too.

use std::panic;
use std::path::{Path, PathBuf};

const FILE: &str = "crash.log";
/// When the log grows past this it is cut back to its newest reports.
const MAX_BYTES: usize = 256 * 1024;
/// What every report starts with, so a cut never leaves half a report.
const MARK: &str = "--- ";

/// Where the log is, if the profile has a folder.
pub fn path() -> Option<PathBuf> {
    Some(crate::profile::app_data()?.join(FILE))
}

/// One report as written to the log.
pub fn report(
    when: &str,
    version: &str,
    thread: &str,
    message: &str,
    place: &str,
    trace: &str,
) -> String {
    format!(
        "{MARK}{when}  OpenDraft {version}  thread {thread}\n{message}\n  at {place}\n{trace}\n\n"
    )
}

/// Add `text` to the log at `path`, keeping the file within [`MAX_BYTES`] by
/// dropping the oldest reports.
pub fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut log = std::fs::read_to_string(path).unwrap_or_default();
    log.push_str(text);
    if log.len() > MAX_BYTES {
        let mut cut = log.len() - MAX_BYTES;
        while !log.is_char_boundary(cut) {
            cut += 1;
        }
        // Start again at the next whole report; if the newest one alone is too
        // long, keep its end.
        let start = log[cut..]
            .find(&format!("\n{MARK}"))
            .map_or(cut, |at| cut + at + 1);
        log = log[start..].to_owned();
    }
    std::fs::write(path, log)
}

/// What a panic said.
fn message_of(info: &panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".to_owned())
}

/// Log every panic to the profile's `crash.log`.
pub fn install() {
    if let Some(path) = path() {
        install_at(path);
    }
}

fn install_at(path: PathBuf) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let place = info.location().map_or_else(
            || "an unknown place".to_owned(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        let thread = std::thread::current();
        let text = report(
            &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            &crate::build_info::display(),
            thread.name().unwrap_or("unnamed"),
            &message_of(info),
            &place,
            &std::backtrace::Backtrace::force_capture().to_string(),
        );
        let _ = append(&path, &text);
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-crash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_report_says_when_which_version_where_and_what() {
        let text = report(
            "2026-10-25 12:00:00",
            "0.1.3",
            "main",
            "boom",
            "src/x.rs:7",
            "frames",
        );
        assert!(text.starts_with("--- 2026-10-25 12:00:00  OpenDraft 0.1.3  thread main\n"));
        assert!(text.contains("\nboom\n  at src/x.rs:7\nframes\n"));
    }

    #[test]
    fn reports_are_added_to_the_end_and_the_folder_is_made() {
        let dir = scratch("append");
        let log = dir.join("deeper").join(FILE);
        append(&log, "--- one\n").unwrap();
        append(&log, "--- two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "--- one\n--- two\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_long_log_loses_its_oldest_reports_whole() {
        let dir = scratch("bound");
        let log = dir.join(FILE);
        let report_of = |n: usize| format!("{MARK}report {n}\n{}\n", "x".repeat(40_000));
        for n in 0..10 {
            append(&log, &report_of(n)).unwrap();
        }
        let kept = std::fs::read_to_string(&log).unwrap();
        assert!(kept.len() <= MAX_BYTES, "{} bytes", kept.len());
        assert!(kept.starts_with(MARK), "a cut must start at a report");
        assert!(kept.contains("report 9"), "the newest report is kept");
        assert!(!kept.contains("report 0\n"), "the oldest is dropped");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_report_longer_than_the_limit_keeps_its_end() {
        let dir = scratch("huge");
        let log = dir.join(FILE);
        append(
            &log,
            &format!("{MARK}huge\n{}END\n", "y".repeat(MAX_BYTES * 2)),
        )
        .unwrap();
        let kept = std::fs::read_to_string(&log).unwrap();
        assert!(kept.len() <= MAX_BYTES && kept.ends_with("END\n"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_panic_is_written_to_the_log() {
        let dir = scratch("hook");
        let log = dir.join(FILE);
        install_at(log.clone());
        let caught = std::thread::Builder::new()
            .name("crash-test".into())
            .spawn(|| panic!("a deliberate test panic"))
            .unwrap()
            .join();
        assert!(caught.is_err());
        // Put the ordinary hook back for the other tests.
        let _ = panic::take_hook();
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("a deliberate test panic"), "{text}");
        assert!(text.contains("thread crash-test"), "{text}");
        assert!(text.contains(" OpenDraft "), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
