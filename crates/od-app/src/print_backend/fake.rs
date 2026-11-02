//! A printer that is not there: a backend for tests that records the jobs it
//! is given and can be told to fail.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use super::{
    resolve_media, validate_job, Capabilities, Medium, PrintBackend, PrintError, PrintJob, Printer,
    PrinterId, Receipt,
};

pub struct FakeBackend {
    pub printers: Vec<Printer>,
    pub media: Vec<Medium>,
    /// What `submit` answers instead of accepting, when set.
    pub fail_with: Mutex<Option<PrintError>>,
    pub jobs: Mutex<Vec<PrintJob>>,
}

impl FakeBackend {
    /// One default printer, "Office", that takes A4.
    pub fn office() -> Self {
        Self {
            printers: vec![Printer {
                id: PrinterId("Office".into()),
                is_default: true,
            }],
            media: vec![Medium {
                name: "A4".into(),
                width_mm: 210.0,
                height_mm: 297.0,
            }],
            fail_with: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
        }
    }
}

impl PrintBackend for FakeBackend {
    fn printers(&self) -> Result<Vec<Printer>, PrintError> {
        Ok(self.printers.clone())
    }

    fn capabilities(&self, printer: &PrinterId) -> Result<Capabilities, PrintError> {
        if self.printers.iter().any(|p| &p.id == printer) {
            Ok(Capabilities {
                media: self.media.clone(),
            })
        } else {
            Err(PrintError::Capability("no such printer".into()))
        }
    }

    fn submit(&self, job: &PrintJob, cancel: &AtomicBool) -> Result<Receipt, PrintError> {
        validate_job(job)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(PrintError::Cancelled);
        }
        resolve_media(&self.capabilities(&job.printer)?, &job.sheet)?;
        if let Some(error) = self
            .fail_with
            .lock()
            .map_err(|_| PrintError::Cancelled)?
            .clone()
        {
            return Err(error);
        }
        let mut jobs = self.jobs.lock().map_err(|_| PrintError::Cancelled)?;
        jobs.push(job.clone());
        Ok(Receipt {
            queue_id: Some(format!("Office-{}", jobs.len())),
        })
    }
}
