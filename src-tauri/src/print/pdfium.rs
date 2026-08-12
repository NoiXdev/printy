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

/// Longest edge, in inches, that a raster image is allowed to keep before
/// `load_image_page` downscales it. PDFs are bounded by their own page size in
/// points, converted through the clamped render DPI (see `render_pages`), but
/// a bitmap image carries no page geometry at all — `image` 0.25 does not
/// expose embedded resolution metadata (TIFF/JFIF/pHYs) through its unified
/// decoding API, so there is no "physical size" to read back from the file.
/// Paper-size selection is out of scope for Printy (design spec §2), so
/// rather than invent a per-file physical size this pins the bound to the
/// largest common office paper format (A3's long edge, 16.54in) — generous
/// enough to leave any real scan or photo at native resolution, while still
/// bounding memory for a mismeasured, absurdly high-DPI scan such as a
/// 1200 dpi "A4" TIFF (9930x14040px, ~418 MB as RGB).
const MAX_IMAGE_LONG_EDGE_IN: f64 = 16.54;

/// Downscales `img` if its longest edge exceeds what `dpi` (already clamped to
/// `MAX_RENDER_DPI` by the caller) justifies for `MAX_IMAGE_LONG_EDGE_IN`,
/// preserving aspect ratio. Images already within bounds are returned
/// untouched, so a normal scan is never needlessly resampled.
fn clamp_image_to_dpi(img: image::RgbImage, dpi: i32) -> image::RgbImage {
    let max_edge = ((MAX_IMAGE_LONG_EDGE_IN * dpi.max(1) as f64).round() as u32).max(1);
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_edge {
        return img;
    }
    let scale = max_edge as f64 / longest as f64;
    let new_w = ((w as f64 * scale).round() as u32).max(1);
    let new_h = ((h as f64 * scale).round() as u32).max(1);
    image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle)
}

/// Loads a raster image file as a single "page", downscaled to respect the
/// same DPI cap PDFs already honour (`MAX_RENDER_DPI`, via `render_dpi`).
pub fn load_image_page(file: &Path, dpi: i32) -> Result<Vec<image::RgbImage>, PrintError> {
    let img = image::open(file).map_err(|e| PrintError::file(format!("Bild nicht lesbar: {e}")))?;
    Ok(vec![clamp_image_to_dpi(img.into_rgb8(), dpi)])
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
        "jpg" | "jpeg" | "png" | "tif" | "tiff" => load_image_page(file, dpi),
        other => Err(PrintError::file(format!(
            "Dateityp nicht unterstützt: .{other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3000x2000 image at 50 dpi: max edge = round(16.54 * 50) = 827px,
    /// well under the source's 3000px long edge — this is the "1200 dpi A4
    /// TIFF" scenario in miniature, scaled down so the test allocates
    /// megabytes instead of hundreds of them.
    #[test]
    fn an_oversized_image_is_downscaled_preserving_aspect_ratio() {
        let img = image::RgbImage::from_pixel(3000, 2000, image::Rgb([10, 20, 30]));
        let out = clamp_image_to_dpi(img, 50);

        assert_eq!(out.width(), 827);
        // height = round(2000 * 827/3000) = round(551.33) = 551
        assert_eq!(out.height(), 551);

        let src_ratio = 3000.0_f64 / 2000.0;
        let out_ratio = out.width() as f64 / out.height() as f64;
        assert!(
            (src_ratio - out_ratio).abs() < 0.01,
            "aspect ratio must be preserved: {src_ratio} vs {out_ratio}"
        );
    }

    /// A small image well within what even a low DPI justifies must be left
    /// completely untouched -- not merely "close", but byte-identical.
    #[test]
    fn a_small_image_is_left_unchanged() {
        let img = image::RgbImage::from_pixel(800, 600, image::Rgb([1, 2, 3]));
        let before = img.clone();
        let out = clamp_image_to_dpi(img, 300);

        assert_eq!(out.dimensions(), before.dimensions());
        assert_eq!(out.as_raw(), before.as_raw());
    }

    #[test]
    fn an_image_exactly_at_the_bound_is_left_unchanged() {
        // max edge at 100 dpi = round(16.54 * 100) = 1654.
        let img = image::RgbImage::from_pixel(1654, 1000, image::Rgb([9, 9, 9]));
        let out = clamp_image_to_dpi(img, 100);
        assert_eq!(out.dimensions(), (1654, 1000));
    }
}
