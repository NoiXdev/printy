use crate::print::{
    PrintBackend, PrintError, PrintErrorKind, PrintRequest, PrinterCapabilities, PrinterInfo,
};
use std::sync::Mutex;

/// Test double. Records every accepted job and can be armed to fail once.
pub struct FakeBackend {
    printers: Vec<String>,
    jobs: Mutex<Vec<PrintRequest>>,
    next_failure: Mutex<Option<PrintError>>,
}

impl FakeBackend {
    pub fn new(printers: &[&str]) -> Self {
        FakeBackend {
            printers: printers.iter().map(|s| s.to_string()).collect(),
            jobs: Mutex::new(Vec::new()),
            next_failure: Mutex::new(None),
        }
    }

    pub fn jobs(&self) -> Vec<PrintRequest> {
        self.jobs.lock().unwrap().clone()
    }

    /// Arms a one-shot failure for the next `print` call.
    pub fn fail_next(&self, kind: PrintErrorKind, message: &str) {
        *self.next_failure.lock().unwrap() =
            Some(PrintError { kind, message: message.to_string() });
    }
}

impl PrintBackend for FakeBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        Ok(self
            .printers
            .iter()
            .enumerate()
            .map(|(i, n)| PrinterInfo { name: n.clone(), is_default: i == 0 })
            .collect())
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if let Some(e) = self.next_failure.lock().unwrap().take() {
            return Err(e);
        }
        self.jobs.lock().unwrap().push(req.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintBackend, PrintErrorKind, PrintRequest};
    use std::path::PathBuf;

    fn req() -> PrintRequest {
        PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "P".into(),
            copies: 2,
            duplex: DuplexMode::LongEdge,
            color: ColorMode::Mono,
        }
    }

    #[test]
    fn records_every_job_it_is_given() {
        let b = FakeBackend::new(&["P", "Q"]);
        b.print(&req()).unwrap();
        b.print(&req()).unwrap();
        assert_eq!(b.jobs().len(), 2);
        assert_eq!(b.jobs()[0].copies, 2);
    }

    #[test]
    fn lists_printers_and_marks_the_first_as_default() {
        let b = FakeBackend::new(&["P", "Q"]);
        let list = b.list_printers().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_default);
        assert!(!list[1].is_default);
    }

    #[test]
    fn can_be_told_to_fail_with_a_chosen_kind() {
        let b = FakeBackend::new(&["P"]);
        b.fail_next(PrintErrorKind::Printer, "Drucker offline");
        let err = b.print(&req()).unwrap_err();
        assert_eq!(err.kind, PrintErrorKind::Printer);
        assert!(b.jobs().is_empty());
        // The failure is one-shot: the next call succeeds again.
        b.print(&req()).unwrap();
        assert_eq!(b.jobs().len(), 1);
    }
}
