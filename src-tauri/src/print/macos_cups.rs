use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintErrorKind, PrintRequest,
    PrinterCapabilities, PrinterInfo,
};
use std::process::Command;

pub struct CupsBackend;

/// Parses `lpstat -p -d` output. Lines look like:
///   printer NAME is idle.  enabled since ...
///   system default destination: NAME
pub fn parse_lpstat(out: &str) -> Vec<PrinterInfo> {
    let mut printers: Vec<PrinterInfo> = Vec::new();
    let mut default_name: Option<String> = None;

    for line in out.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("printer ") {
            if let Some(name) = rest.split_whitespace().next() {
                printers.push(PrinterInfo { name: name.to_string(), is_default: false });
            }
        } else if let Some(rest) = line.strip_prefix("system default destination: ") {
            default_name = Some(rest.trim().to_string());
        }
    }
    if let Some(d) = default_name {
        for p in printers.iter_mut() {
            p.is_default = p.name == d;
        }
    }
    printers
}

/// Builds the argument vector for `lp`. Pure, so it is unit-testable.
///
/// `fit_to_page = true` appends CUPS' own `-o fit-to-page`, which scales
/// content to fill the media, upscaling when necessary. There is no CUPS
/// option for "shrink to fit but never enlarge", so `fit_to_page = false`
/// emits nothing extra: a PDF then prints at its embedded page size (and is
/// clipped, not shrunk, if oversized) while several CUPS image filters
/// already downscale oversized images by default. This asymmetry between
/// PDFs and images when the flag is off is a known limitation of this
/// development-only backend; the Windows GDI backend (Task 8) does not share
/// it, since it rasterises and places pixels itself via `fit_centered`.
pub fn lp_args(req: &PrintRequest) -> Vec<String> {
    let sides = match req.duplex {
        DuplexMode::Simplex => "sides=one-sided",
        DuplexMode::LongEdge => "sides=two-sided-long-edge",
        DuplexMode::ShortEdge => "sides=two-sided-short-edge",
    };
    let color = match req.color {
        ColorMode::Color => "ColorModel=RGB",
        ColorMode::Mono => "ColorModel=Gray",
    };
    let mut args = vec![
        "-d".to_string(), req.printer.clone(),
        "-n".to_string(), req.copies.to_string(),
        "-o".to_string(), sides.to_string(),
        "-o".to_string(), color.to_string(),
    ];
    if req.fit_to_page {
        args.push("-o".to_string());
        args.push("fit-to-page".to_string());
    }
    args.push(req.file.to_string_lossy().to_string());
    args
}

/// Classifies `lp`'s stderr. A missing or stopped destination is a printer-level
/// problem that must hold the queue; anything else fails just this job.
pub fn classify_stderr(stderr: &str) -> PrintErrorKind {
    if stderr.contains("does not exist") || stderr.contains("not accepting") {
        PrintErrorKind::Printer
    } else {
        PrintErrorKind::File
    }
}

impl PrintBackend for CupsBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        let out = Command::new("lpstat")
            .args(["-p", "-d"])
            // Force C locale to ensure English output regardless of system language.
            // CUPS translates its stderr messages based on the system locale, which
            // would break the error classification in the print method.
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| PrintError::config(format!("lpstat nicht ausführbar: {e}")))?;
        Ok(parse_lpstat(&String::from_utf8_lossy(&out.stdout)))
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        // CUPS reports capabilities through PPD options; probing them adds no
        // value on the development platform, so everything is offered.
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if !req.file.exists() {
            return Err(PrintError::file(format!(
                "Datei nicht gefunden: {}", req.file.display()
            )));
        }
        let out = Command::new("lp")
            .args(lp_args(req))
            // Force C locale to ensure English output regardless of system language.
            // CUPS translates its stderr messages based on the system locale, which
            // would break the error classification below.
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| PrintError::printer(format!("lp nicht ausführbar: {e}")))?;
        if out.status.success() {
            return Ok(());
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let kind = classify_stderr(&msg);
        if kind == PrintErrorKind::Printer {
            Err(PrintError::printer(format!("Drucker nicht erreichbar: {msg}")))
        } else {
            Err(PrintError::file(format!("Druck fehlgeschlagen: {msg}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintRequest};
    use std::path::PathBuf;

    #[test]
    fn parses_lpstat_output_and_marks_the_default() {
        let out = "printer Brother_MFC is idle.  enabled since Mon\n\
                   printer HP_LaserJet is idle.  enabled since Mon\n\
                   system default destination: HP_LaserJet\n";
        let list = parse_lpstat(out);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Brother_MFC");
        assert!(!list[0].is_default);
        assert!(list[1].is_default);
    }

    #[test]
    fn returns_no_printers_for_empty_output() {
        assert!(parse_lpstat("").is_empty());
    }

    #[test]
    fn builds_lp_arguments_for_duplex_mono() {
        // fit_to_page = false: no CUPS fit-to-page flag is emitted.
        let req = PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "HP".into(),
            copies: 3,
            duplex: DuplexMode::LongEdge,
            color: ColorMode::Mono,
            fit_to_page: false,
        };
        assert_eq!(
            lp_args(&req),
            vec![
                "-d", "HP",
                "-n", "3",
                "-o", "sides=two-sided-long-edge",
                "-o", "ColorModel=Gray",
                "/tmp/a.pdf",
            ]
        );
    }

    #[test]
    fn builds_lp_arguments_for_simplex_color() {
        // fit_to_page = false: no CUPS fit-to-page flag is emitted.
        let req = PrintRequest {
            file: PathBuf::from("/tmp/b.png"),
            printer: "P".into(),
            copies: 1,
            duplex: DuplexMode::Simplex,
            color: ColorMode::Color,
            fit_to_page: false,
        };
        assert_eq!(
            lp_args(&req),
            vec![
                "-d", "P",
                "-n", "1",
                "-o", "sides=one-sided",
                "-o", "ColorModel=RGB",
                "/tmp/b.png",
            ]
        );
    }

    #[test]
    fn builds_lp_arguments_with_fit_to_page_appends_the_cups_flag() {
        // fit_to_page = true: CUPS' own scale-to-fill option is appended
        // after the duplex/color options and before the file path.
        let req = PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "HP".into(),
            copies: 1,
            duplex: DuplexMode::Simplex,
            color: ColorMode::Mono,
            fit_to_page: true,
        };
        assert_eq!(
            lp_args(&req),
            vec![
                "-d", "HP",
                "-n", "1",
                "-o", "sides=one-sided",
                "-o", "ColorModel=Gray",
                "-o", "fit-to-page",
                "/tmp/a.pdf",
            ]
        );
    }

    #[test]
    fn classifies_missing_destination_as_printer_error() {
        let stderr = "lp: Error - The printer or class does not exist.";
        assert_eq!(classify_stderr(stderr), PrintErrorKind::Printer);
    }

    #[test]
    fn classifies_not_accepting_as_printer_error() {
        let stderr = "lp: Error - Destination \"HP\" is not accepting jobs.";
        assert_eq!(classify_stderr(stderr), PrintErrorKind::Printer);
    }

    #[test]
    fn classifies_unrelated_failure_as_file_error() {
        let stderr = "lp: Error - Unable to open file: No such file or directory";
        assert_eq!(classify_stderr(stderr), PrintErrorKind::File);
    }
}
