use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintErrorKind, PrintRequest,
    PrinterCapabilities, PrinterInfo,
};
use std::process::Command;

pub struct CupsBackend;

/// Parses `lpstat -e` output: one destination name per line, no surrounding
/// prose and — unlike `lpstat -p -d` — no localisation, so this is the only
/// safe source of printer names on macOS.
pub fn parse_printer_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Resolves the default destination out of `lpstat -d`'s output, which is
/// localised prose on macOS regardless of language. The destination name is
/// always the last whitespace-separated token of the line in every language
/// observed (English `system default destination: NAME`, German
/// `System-Standardzielort: NAME`), so we take that token and accept it only
/// if it also appears in `names` (from `lpstat -e`). That membership check is
/// what makes this safe: when there is no default, the line is a sentence
/// (e.g. "kein System-Standardzielort") whose last token will not match any
/// real destination, so it correctly resolves to `None` instead of a bogus name.
pub fn resolve_default(default_line: &str, names: &[String]) -> Option<String> {
    let last = default_line.split_whitespace().last()?;
    names.iter().find(|n| n.as_str() == last).cloned()
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
///
/// `stderr` text is localised on macOS just like `lpstat`'s, so it cannot be
/// the sole signal: the primary check is whether `printer` still shows up in
/// `live_destinations` (a fresh `lpstat -e` snapshot taken right after the
/// failure). If it is absent, the destination is gone or disabled — `Printer`.
/// If it is present, the English substrings are kept only as an *additional*
/// way to reach `Printer` (e.g. a destination that is technically still
/// enumerated but was just rejected outright); on a localised system they will
/// simply never match, and the destination-membership check still applies.
pub fn classify_stderr(stderr: &str, printer: &str, live_destinations: &[String]) -> PrintErrorKind {
    let still_listed = live_destinations.iter().any(|d| d == printer);
    if !still_listed {
        return PrintErrorKind::Printer;
    }
    if stderr.contains("does not exist") || stderr.contains("not accepting") {
        PrintErrorKind::Printer
    } else {
        PrintErrorKind::File
    }
}

impl PrintBackend for CupsBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        // `-e` lists destination names only, with no localised prose around
        // them, so it is the only reliable source of names on macOS.
        let names_out = Command::new("lpstat")
            .arg("-e")
            // NOTE: `LC_ALL=C` helps on Linux but is NOT sufficient on macOS —
            // Apple's CUPS tools translate via the system language regardless
            // of this variable. It is kept because it is harmless and does
            // help elsewhere; do not rely on it here to get English prose.
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| PrintError::config(format!("lpstat nicht ausführbar: {e}")))?;
        let names = parse_printer_names(&String::from_utf8_lossy(&names_out.stdout));

        let default_out = Command::new("lpstat")
            .arg("-d")
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| PrintError::config(format!("lpstat nicht ausführbar: {e}")))?;
        let default_line = String::from_utf8_lossy(&default_out.stdout);
        let default = resolve_default(&default_line, &names);

        Ok(names
            .into_iter()
            .map(|name| {
                let is_default = default.as_deref() == Some(name.as_str());
                PrinterInfo { name, is_default }
            })
            .collect())
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
            // NOTE: `LC_ALL=C` helps on Linux but is NOT sufficient on macOS —
            // Apple's CUPS tools translate stderr via the system language
            // regardless of this variable. classify_stderr below therefore
            // does not rely on English substrings alone; see its doc comment.
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| PrintError::printer(format!("lp nicht ausführbar: {e}")))?;
        if out.status.success() {
            return Ok(());
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // Re-check whether the destination is still enumerated right after the
        // failure. This is locale-independent, unlike matching stderr prose.
        let live_destinations = Command::new("lpstat")
            .arg("-e")
            .env("LC_ALL", "C")
            .output()
            .map(|o| parse_printer_names(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default();
        let kind = classify_stderr(&msg, &req.printer, &live_destinations);
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

    // Real output captured on the affected German-locale Mac. `lpstat -e` is
    // never localised, which is exactly why it — and not `lpstat -p -d` — is
    // the source of truth for printer names.
    const REAL_LPSTAT_E: &str = "A3B___Develop_ineoPlus\n\
        HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_\n";

    // Real `lpstat -d` output captured on the same German-locale Mac.
    const REAL_GERMAN_DEFAULT_LINE: &str =
        "System-Standardzielort: HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_\n";

    // Real `lpstat -p -d` prose captured on the same machine — the format the
    // old, now-deleted `parse_lpstat` depended on and which broke silently
    // under German localisation.
    const REAL_GERMAN_LPSTAT_P_D: &str = "Drucker „A3B___Develop_ineoPlus“ ist inaktiv; aktiviert seit Mon Aug  3 07:13:51 2026\n\
        Drucker „HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_“ ist inaktiv; aktiviert seit Sun Aug  9 18:17:48 2026\n\
        System-Standardzielort: HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_\n";

    #[test]
    fn parse_printer_names_returns_both_names_from_real_lpstat_e_output() {
        let names = parse_printer_names(REAL_LPSTAT_E);
        assert_eq!(
            names,
            vec![
                "A3B___Develop_ineoPlus".to_string(),
                "HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_".to_string(),
            ]
        );
    }

    #[test]
    fn parse_printer_names_skips_blank_lines_and_trims() {
        assert!(parse_printer_names("").is_empty());
        assert_eq!(parse_printer_names("\n  A  \n\n B \n"), vec!["A", "B"]);
    }

    #[test]
    fn resolve_default_extracts_name_from_real_german_default_line() {
        let names = parse_printer_names(REAL_LPSTAT_E);
        let default = resolve_default(REAL_GERMAN_DEFAULT_LINE, &names);
        assert_eq!(
            default,
            Some("HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_".to_string())
        );
    }

    #[test]
    fn resolve_default_returns_none_when_last_token_is_not_a_known_destination() {
        let names = parse_printer_names(REAL_LPSTAT_E);
        // A plausible "no default set" message in a non-English language
        // (French here, distinct from the German fixture above): its last
        // token is an ordinary word, not a destination name.
        let no_default_line = "Espace de destination par défaut du système : aucun";
        assert_eq!(resolve_default(no_default_line, &names), None);
    }

    #[test]
    fn old_localized_lpstat_p_d_prose_does_not_yield_real_printer_names() {
        // The new parsing path never looks at `lpstat -p -d` output at all.
        // Feeding it through parse_printer_names anyway (as if it were fed to
        // the old code's input) must not produce the real destination names,
        // proving the new code does not depend on that format.
        let names = parse_printer_names(REAL_GERMAN_LPSTAT_P_D);
        assert!(!names.contains(&"A3B___Develop_ineoPlus".to_string()));
        assert!(
            !names.contains(&"HP7C4D8F7C1334__HP_Color_Laser_MFP_178_179_".to_string())
        );
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
    fn classifies_as_printer_error_when_destination_no_longer_listed() {
        // Locale-independent path: the destination vanished from a fresh
        // `lpstat -e`, so it does not matter what stderr says (here: German
        // prose, which the old substring check could never have matched).
        let stderr = "lp: Fehler - Der Drucker oder die Klasse ist nicht vorhanden.";
        let live: Vec<String> = vec!["OtherPrinter".to_string()];
        assert_eq!(
            classify_stderr(stderr, "HP", &live),
            PrintErrorKind::Printer
        );
    }

    #[test]
    fn classifies_as_file_error_when_destination_still_listed_and_stderr_is_unrelated() {
        let stderr = "lp: Error - Unable to open file: No such file or directory";
        let live = vec!["HP".to_string()];
        assert_eq!(classify_stderr(stderr, "HP", &live), PrintErrorKind::File);
    }

    #[test]
    fn english_does_not_exist_substring_is_an_additional_path_to_printer_error() {
        // Destination is still enumerated, but the English substring check
        // is kept as an extra way to reach `Printer` — never the sole basis.
        let stderr = "lp: Error - The printer or class does not exist.";
        let live = vec!["HP".to_string()];
        assert_eq!(
            classify_stderr(stderr, "HP", &live),
            PrintErrorKind::Printer
        );
    }

    #[test]
    fn english_not_accepting_substring_is_an_additional_path_to_printer_error() {
        let stderr = "lp: Error - Destination \"HP\" is not accepting jobs.";
        let live = vec!["HP".to_string()];
        assert_eq!(
            classify_stderr(stderr, "HP", &live),
            PrintErrorKind::Printer
        );
    }
}
