use crate::print::{ColorMode, DuplexMode, PrintRequest};
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use crate::print::{PrintBackend, PrintError, PrinterCapabilities, PrinterInfo};
#[cfg(target_os = "windows")]
use std::process::Command;

/// Standard install locations, checked only when no path is configured.
const STANDARD_PATHS: &[&str] = &[
    r"C:\Program Files\SumatraPDF\SumatraPDF.exe",
    r"C:\Program Files (x86)\SumatraPDF\SumatraPDF.exe",
];

/// SumatraPDF is never bundled — doing so would attach GPLv3 obligations to the
/// installer. Only an independently installed copy is used.
pub fn find_sumatra(configured: Option<String>) -> Option<PathBuf> {
    if let Some(p) = configured {
        let p = PathBuf::from(p);
        return if p.exists() { Some(p) } else { None };
    }
    STANDARD_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// Builds the `-print-settings` value.
pub fn sumatra_settings(req: &PrintRequest) -> String {
    let mut parts: Vec<String> = Vec::new();
    if req.copies > 1 {
        parts.push(format!("{}x", req.copies));
    }
    parts.push(
        match req.duplex {
            DuplexMode::Simplex => "simplex",
            DuplexMode::LongEdge => "duplexlong",
            DuplexMode::ShortEdge => "duplexshort",
        }
        .to_string(),
    );
    parts.push(
        match req.color {
            ColorMode::Color => "color",
            ColorMode::Mono => "monochrome",
        }
        .to_string(),
    );
    parts.join(",")
}

#[cfg(target_os = "windows")]
pub struct SumatraBackend {
    pub exe: PathBuf,
}

#[cfg(target_os = "windows")]
impl PrintBackend for SumatraBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        // Sumatra is a fallback for rendering only; it never enumerates.
        Err(PrintError::config(
            "SumatraPDF liefert keine Druckerliste".to_string(),
        ))
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        let out = Command::new(&self.exe)
            .arg("-print-to").arg(&req.printer)
            .arg("-silent")
            .arg("-print-settings").arg(sumatra_settings(req))
            .arg(&req.file)
            .output()
            .map_err(|e| PrintError::file(format!("SumatraPDF nicht startbar: {e}")))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(PrintError::file(format!(
                "SumatraPDF-Druck fehlgeschlagen (Code {:?})",
                out.status.code()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintRequest};
    use std::path::PathBuf;

    fn req(copies: u32, duplex: DuplexMode, color: ColorMode) -> PrintRequest {
        PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "HP".into(),
            copies,
            duplex,
            color,
            fit_to_page: true,
        }
    }

    #[test]
    fn builds_settings_string_for_duplex_mono_multiple_copies() {
        let s = sumatra_settings(&req(3, DuplexMode::LongEdge, ColorMode::Mono));
        assert_eq!(s, "3x,duplexlong,monochrome");
    }

    #[test]
    fn omits_copies_when_only_one_is_requested() {
        let s = sumatra_settings(&req(1, DuplexMode::Simplex, ColorMode::Color));
        assert_eq!(s, "simplex,color");
    }

    #[test]
    fn short_edge_maps_to_duplexshort() {
        let s = sumatra_settings(&req(1, DuplexMode::ShortEdge, ColorMode::Color));
        assert_eq!(s, "duplexshort,color");
    }

    #[test]
    fn configured_path_wins_over_the_standard_locations() {
        let here = std::env::current_exe().unwrap();
        let found = find_sumatra(Some(here.to_string_lossy().to_string()));
        assert_eq!(found, Some(here));
    }

    #[test]
    fn a_configured_path_that_does_not_exist_is_ignored() {
        assert!(find_sumatra(Some("/nope/SumatraPDF.exe".into())).is_none());
    }
}
