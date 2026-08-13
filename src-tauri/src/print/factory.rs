use crate::print::PrintBackend;

/// Selects the platform backend. On Windows the GDI backend is primary and
/// SumatraPDF, if installed, is consulted only for rendering failures — inside
/// the same attempt, never for printer-level errors. `pdfium_path` is the
/// user's configured pdfium location, threaded down to the rasteriser.
#[cfg(target_os = "windows")]
pub fn backend(sumatra_path: Option<String>, pdfium_path: Option<String>) -> Box<dyn PrintBackend> {
    use crate::print::sumatra::{find_sumatra, SumatraBackend};
    use crate::print::windows_gdi::GdiBackend;
    use crate::print::{PrintError, PrintErrorKind, PrintRequest, PrinterCapabilities, PrinterInfo};

    struct WithFallback {
        primary: GdiBackend,
        fallback: Option<SumatraBackend>,
    }

    impl PrintBackend for WithFallback {
        fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
            self.primary.list_printers()
        }
        fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError> {
            self.primary.capabilities(printer)
        }
        fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
            match self.primary.print(req) {
                Ok(()) => Ok(()),
                // Printer-level problems are not rendering problems.
                Err(e) if e.kind == PrintErrorKind::Printer => Err(e),
                Err(e) => match &self.fallback {
                    Some(f) => f.print(req),
                    None => Err(e),
                },
            }
        }
    }

    Box::new(WithFallback {
        primary: GdiBackend { pdfium_path },
        fallback: find_sumatra(sumatra_path).map(|exe| SumatraBackend { exe }),
    })
}

#[cfg(target_os = "macos")]
pub fn backend(_sumatra_path: Option<String>, _pdfium_path: Option<String>) -> Box<dyn PrintBackend> {
    Box::new(crate::print::macos_cups::CupsBackend)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn backend(_sumatra_path: Option<String>, _pdfium_path: Option<String>) -> Box<dyn PrintBackend> {
    Box::new(crate::print::fake::FakeBackend::new(&[]))
}
