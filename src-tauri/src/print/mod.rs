pub mod fake;
#[cfg(target_os = "macos")]
pub mod macos_cups;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplexMode {
    Simplex,
    LongEdge,
    ShortEdge,
}

impl DuplexMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "long_edge" => DuplexMode::LongEdge,
            "short_edge" => DuplexMode::ShortEdge,
            _ => DuplexMode::Simplex,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DuplexMode::Simplex => "simplex",
            DuplexMode::LongEdge => "long_edge",
            DuplexMode::ShortEdge => "short_edge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    Color,
    Mono,
}

impl ColorMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "color" => ColorMode::Color,
            _ => ColorMode::Mono,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Color => "color",
            ColorMode::Mono => "mono",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PrinterInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PrinterCapabilities {
    pub duplex: bool,
    pub color: bool,
    pub copies: bool,
}

#[derive(Debug, Clone)]
pub struct PrintRequest {
    pub file: PathBuf,
    pub printer: String,
    pub copies: u32,
    pub duplex: DuplexMode,
    pub color: ColorMode,
}

/// Decides whether one job fails or the whole queue holds. See spec section 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintErrorKind {
    File,
    Printer,
    Config,
}

impl PrintErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PrintErrorKind::File => "file",
            PrintErrorKind::Printer => "printer",
            PrintErrorKind::Config => "config",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct PrintError {
    pub kind: PrintErrorKind,
    pub message: String,
}

impl PrintError {
    pub fn file(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::File, message: msg.into() }
    }
    pub fn printer(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::Printer, message: msg.into() }
    }
    pub fn config(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::Config, message: msg.into() }
    }
}

/// Implementations must be usable from `spawn_blocking`, so `Send + Sync`.
/// They must not hold a `Pdfium` handle; bind it inside `print`.
pub trait PrintBackend: Send + Sync {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError>;
    fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError>;
    fn print(&self, req: &PrintRequest) -> Result<(), PrintError>;
}
