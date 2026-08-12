//! Shared rasteriser. Turns a PDF or a raster image file into a list of
//! RGB bitmaps that a platform backend can blit onto a page.

use crate::print::PrintError;
use pdfium_render::prelude::*;
use std::path::Path;

/// Binds libpdfium from the candidate directories: the working directory (dev
/// and `cargo test`), the executable's own directory, and the two macOS bundle
/// locations. `pdfium_platform_library_name_at_path` yields the OS-correct file
/// name (`pdfium.dll` on Windows, `libpdfium.dylib` on macOS), so no `cfg` is
/// needed here.
pub fn bind_pdfium() -> Result<Pdfium, PrintError> {
    let mut dirs: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from(".")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("../Frameworks"));
            dirs.push(d.join("../Resources"));
        }
    }
    for dir in &dirs {
        let name = Pdfium::pdfium_platform_library_name_at_path(dir);
        if let Ok(b) = Pdfium::bind_to_library(name) {
            return Ok(Pdfium::new(b));
        }
    }
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| PrintError::config(format!("pdfium nicht ladbar: {e}")))
}

/// Rasterises every page of a PDF at `dpi`. Synchronous on purpose: `Pdfium` is
/// not `Send`, so the handle must never cross an await point.
pub fn render_pages(file: &Path, dpi: i32) -> Result<Vec<image::RgbImage>, PrintError> {
    let pdfium = bind_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(file, None)
        .map_err(|e| PrintError::file(format!("PDF nicht lesbar: {e}")))?;

    let mut out = Vec::new();
    for page in doc.pages().iter() {
        // PDF user space is 1/72 inch per unit, so points -> pixels is a
        // straight ratio against the target dpi.
        let width_pt = page.width().value as f64;
        let height_pt = page.height().value as f64;
        let target_w = ((width_pt / 72.0) * dpi as f64).round() as i32;
        let target_h = ((height_pt / 72.0) * dpi as f64).round() as i32;
        let cfg = PdfRenderConfig::new()
            .set_target_width(target_w.max(1))
            .set_maximum_height(target_h.max(1));
        let bitmap = page
            .render_with_config(&cfg)
            .map_err(|e| PrintError::file(format!("Render-Fehler: {e}")))?;
        out.push(bitmap.as_image().into_rgb8());
    }
    if out.is_empty() {
        return Err(PrintError::file("PDF enthält keine Seiten".to_string()));
    }
    Ok(out)
}

/// Loads a raster image file as a single "page".
pub fn load_image_page(file: &Path) -> Result<Vec<image::RgbImage>, PrintError> {
    let img = image::open(file).map_err(|e| PrintError::file(format!("Bild nicht lesbar: {e}")))?;
    Ok(vec![img.into_rgb8()])
}

/// Dispatches by extension. PDF and images converge on one bitmap pipeline.
pub fn rasterise(file: &Path, dpi: i32) -> Result<Vec<image::RgbImage>, PrintError> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => render_pages(file, dpi),
        "jpg" | "jpeg" | "png" | "tif" | "tiff" => load_image_page(file),
        other => Err(PrintError::file(format!(
            "Dateityp nicht unterstützt: .{other}"
        ))),
    }
}
