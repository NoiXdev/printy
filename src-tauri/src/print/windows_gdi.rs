//! Windows printing via GDI. Content is rasterised by pdfium (or the `image`
//! crate) and blitted onto the printer device context with `StretchDIBits`,
//! which keeps placement under our own control instead of the driver's.
//!
//! This module is only compiled on Windows; it is cross-checked from macOS
//! with `cargo check --target x86_64-pc-windows-msvc`.

use crate::print::layout::{fit_centered, render_dpi};
use crate::print::pdfium::rasterise;
use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintRequest, PrinterCapabilities, PrinterInfo,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DEVMODEW, DIB_RGB_COLORS, DMCOLLATE_TRUE, DMCOLOR_COLOR, DMCOLOR_MONOCHROME,
    DMDUP_HORIZONTAL, DMDUP_SIMPLEX, DMDUP_VERTICAL, DM_COLLATE, DM_COLOR, DM_COPIES, DM_DUPLEX,
    DM_IN_BUFFER, DM_OUT_BUFFER, GDI_ERROR, HDC, HORZRES, LOGPIXELSX, SRCCOPY, VERTRES,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW,
    PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
};
use windows::Win32::Storage::Xps::{
    AbortDoc, DeviceCapabilitiesW, EndDoc, EndPage, StartDocW, StartPage, DC_COLORDEVICE, DC_COPIES,
    DC_DUPLEX, DOCINFOW, PRINTER_DEVICE_CAPABILITIES,
};

pub struct GdiBackend {
    /// The user's configured `pdfium_path` setting, forwarded to `rasterise`.
    pub pdfium_path: Option<String>,
}

/// A heap buffer with 8-byte alignment. Win32 writes structures containing
/// pointers (`PRINTER_INFO_4W`) and 4-byte scalars (`DEVMODEW`) into these
/// buffers, while a plain `Vec<u8>` only guarantees 1-byte alignment — which
/// would make the pointer casts below unsound.
struct AlignedBuffer {
    words: Vec<u64>,
    len: usize,
}

impl AlignedBuffer {
    fn new(len: usize) -> Self {
        AlignedBuffer {
            words: vec![0u64; (len + 7) / 8 + 1],
            len,
        }
    }

    fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY: `words` owns at least `len` bytes, and `u64` has no padding
        // and no invalid bit patterns, so viewing it as bytes is well defined.
        unsafe { std::slice::from_raw_parts_mut(self.words.as_mut_ptr().cast::<u8>(), self.len) }
    }

    fn as_ptr(&self) -> *const u8 {
        self.words.as_ptr().cast::<u8>()
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast::<u8>()
    }
}

/// NUL-terminated UTF-16, the form every `...W` entry point expects.
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Decodes a UTF-16 buffer up to its first NUL.
fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn default_printer_name() -> Option<String> {
    unsafe {
        let mut len: u32 = 0;
        // The sizing call is expected to fail; it reports the buffer length in
        // characters, including the terminating NUL.
        let _ = GetDefaultPrinterW(PWSTR::null(), &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        if !GetDefaultPrinterW(PWSTR(buf.as_mut_ptr()), &mut len).as_bool() {
            return None;
        }
        Some(from_wide(&buf))
    }
}

/// Queries one `DC_*` capability. Returns a negative value when the driver
/// cannot answer, which callers treat as "not supported".
unsafe fn device_cap(name: &[u16], what: PRINTER_DEVICE_CAPABILITIES) -> i32 {
    DeviceCapabilitiesW(
        PCWSTR(name.as_ptr()),
        PCWSTR::null(),
        what,
        PWSTR::null(),
        None,
    )
}

/// Fetches the driver's DEVMODE, patches in the job's copies/duplex/colour and
/// lets the driver normalise the result. The caller owns the returned buffer
/// and must keep it alive for as long as the pointer is handed to Win32.
unsafe fn build_devmode(
    handle: HANDLE,
    name: &[u16],
    req: &PrintRequest,
) -> Result<AlignedBuffer, PrintError> {
    let size = DocumentPropertiesW(None, handle, PCWSTR(name.as_ptr()), None, None, 0);
    // `size` covers `DEVMODEW` plus the driver's private `dmDriverExtra` tail;
    // anything shorter than the struct itself would make the writes below
    // overrun the buffer.
    if size <= 0 || (size as usize) < std::mem::size_of::<DEVMODEW>() {
        return Err(PrintError::printer(
            "Treiber liefert keine Einstellungen".to_string(),
        ));
    }

    let mut buf = AlignedBuffer::new(size as usize);
    let dm = buf.as_mut_ptr().cast::<DEVMODEW>();
    if DocumentPropertiesW(
        None,
        handle,
        PCWSTR(name.as_ptr()),
        Some(dm),
        None,
        DM_OUT_BUFFER.0,
    ) < 0
    {
        return Err(PrintError::printer(
            "Treibereinstellungen nicht lesbar".to_string(),
        ));
    }

    (*dm).dmFields |= DM_COPIES | DM_DUPLEX | DM_COLOR | DM_COLLATE;
    // `dmCopies` lives in the first anonymous union; `dmDuplex`, `dmColor` and
    // `dmCollate` are plain fields of DEVMODEW.
    (*dm).Anonymous1.Anonymous1.dmCopies = req.copies.clamp(1, i16::MAX as u32) as i16;
    (*dm).dmDuplex = match req.duplex {
        DuplexMode::Simplex => DMDUP_SIMPLEX,
        DuplexMode::LongEdge => DMDUP_VERTICAL,
        DuplexMode::ShortEdge => DMDUP_HORIZONTAL,
    };
    (*dm).dmColor = match req.color {
        ColorMode::Color => DMCOLOR_COLOR,
        ColorMode::Mono => DMCOLOR_MONOCHROME,
    };
    // Without this, multi-copy jobs come out in whatever page order the
    // driver defaults to — three copies of a three-page document could print
    // as page1 x3, page2 x3, page3 x3 instead of three complete sets.
    (*dm).dmCollate = DMCOLLATE_TRUE;

    // Merge pass: the driver validates the patched fields and silently drops
    // anything the device cannot do. Failure here is not fatal — the DEVMODE
    // is still usable, just unnormalised.
    let _ = DocumentPropertiesW(
        None,
        handle,
        PCWSTR(name.as_ptr()),
        Some(dm),
        Some(dm.cast_const()),
        (DM_IN_BUFFER | DM_OUT_BUFFER).0,
    );
    Ok(buf)
}

/// Opens a device context configured with the request's DEVMODE settings.
/// Returns the DC and the device's logical DPI. The caller owns the DC and
/// must `DeleteDC` it.
unsafe fn open_dc(req: &PrintRequest) -> Result<(HDC, i32), PrintError> {
    let name = wide(&req.printer);
    let mut handle = HANDLE::default();
    OpenPrinterW(PCWSTR(name.as_ptr()), &mut handle, None)
        .map_err(|e| PrintError::printer(format!("Drucker nicht erreichbar: {e}")))?;

    // Every path past OpenPrinterW must close the handle before returning.
    let devmode = build_devmode(handle, &name, req);
    let _ = ClosePrinter(handle);
    let devmode = devmode?;

    let hdc = CreateDCW(
        None,
        PCWSTR(name.as_ptr()),
        None,
        Some(devmode.as_ptr().cast::<DEVMODEW>()),
    );
    if hdc.is_invalid() {
        // Spec §9.2: Sumatra is consulted "when a rendering fails, or when no
        // device context can be created" — so this must be a File error, not
        // a Printer error, or factory::backend's fallback branch (which only
        // triggers for non-Printer kinds) can never be reached and a single
        // bad device context holds the entire queue instead of falling back.
        return Err(PrintError::file(
            "Kein Gerätekontext für den Drucker".to_string(),
        ));
    }
    Ok((hdc, GetDeviceCaps(hdc, LOGPIXELSX)))
}

/// Converts an RGB image into a 24-bit DIB pixel buffer. GDI wants BGR channel
/// order and every scanline padded to a 4-byte boundary; skipping the padding
/// skews the image on any width where `width * 3` is not a multiple of four.
/// Returns the buffer and its scanline stride in bytes.
fn to_dib_bgr(page: &image::RgbImage) -> (Vec<u8>, usize) {
    let w = page.width() as usize;
    let h = page.height() as usize;
    let stride = (w * 3 + 3) & !3;
    let src = page.as_raw();
    let mut dib = vec![0u8; stride * h];
    for y in 0..h {
        let src_row = y * w * 3;
        let dst_row = y * stride;
        for x in 0..w {
            let s = src_row + x * 3;
            let d = dst_row + x * 3;
            dib[d] = src[s + 2];
            dib[d + 1] = src[s + 1];
            dib[d + 2] = src[s];
        }
    }
    (dib, stride)
}

/// Emits one page onto an already-started document.
unsafe fn print_one_page(
    hdc: HDC,
    page: &image::RgbImage,
    area_w: i32,
    area_h: i32,
    fit_to_page: bool,
) -> Result<(), PrintError> {
    let w = page.width() as i32;
    let h = page.height() as i32;
    if w <= 0 || h <= 0 {
        return Err(PrintError::file("Leere Seite im Dokument".to_string()));
    }
    // `fit_to_page` is the job's own snapshot, not a constant: a folder set to
    // natural-size printing must not have small labels blown up to full page.
    let rect = fit_centered(w, h, area_w, area_h, fit_to_page);
    if rect.width <= 0 || rect.height <= 0 {
        return Err(PrintError::file(
            "Seite passt nicht auf das Papier".to_string(),
        ));
    }

    let (dib, stride) = to_dib_bgr(page);
    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // Negative height declares a top-down DIB, matching the row order
            // the `image` crate produces.
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 24,
            biCompression: BI_RGB.0,
            biSizeImage: (stride * h as usize) as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    if StartPage(hdc) <= 0 {
        return Err(PrintError::printer("Seite abgelehnt".to_string()));
    }
    let copied = StretchDIBits(
        hdc,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        0,
        0,
        w,
        h,
        Some(dib.as_ptr().cast::<core::ffi::c_void>()),
        &bi,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
    // On success this is the number of scanlines copied, which some printer
    // drivers legitimately report as 0. Only GDI_ERROR signals a real failure.
    // This is a rendering problem, not a printer problem (spec §9.2), so it is
    // classified as File: it lets the Sumatra fallback be tried for this job
    // instead of holding the whole queue for what is really a bad blit.
    if copied == GDI_ERROR {
        return Err(PrintError::file(
            "Seiteninhalt konnte nicht übertragen werden".to_string(),
        ));
    }
    if EndPage(hdc) <= 0 {
        return Err(PrintError::printer(
            "Seite konnte nicht abgeschlossen werden".to_string(),
        ));
    }
    Ok(())
}

/// Runs the whole job on an open DC. Split out so the caller can delete the DC
/// on every path, success or failure.
unsafe fn print_on_dc(
    hdc: HDC,
    device_dpi: i32,
    req: &PrintRequest,
    pdfium_path: Option<&str>,
) -> Result<(), PrintError> {
    let pages = rasterise(&req.file, render_dpi(device_dpi), pdfium_path)?;

    // HORZRES/VERTRES give the printable area in device pixels, and a printer
    // DC's origin already sits at its top-left corner, so no offset correction
    // is needed. The PHYSICAL* indices are deliberately unused: drivers that do
    // not support them return 0, which would produce a negative area and
    // silently print nothing.
    let area_w = GetDeviceCaps(hdc, HORZRES);
    let area_h = GetDeviceCaps(hdc, VERTRES);
    if area_w <= 0 || area_h <= 0 {
        return Err(PrintError::printer(
            "Druckbereich nicht ermittelbar".to_string(),
        ));
    }

    let doc_name = wide(
        req.file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Printy"),
    );
    let di = DOCINFOW {
        cbSize: std::mem::size_of::<DOCINFOW>() as i32,
        lpszDocName: PCWSTR(doc_name.as_ptr()),
        ..Default::default()
    };
    if StartDocW(hdc, &di) <= 0 {
        return Err(PrintError::printer("Druckauftrag abgelehnt".to_string()));
    }

    // Past this point the spooler holds an open document; every failure has to
    // abort it rather than leave a half-written job in the queue.
    for page in &pages {
        if let Err(e) = print_one_page(hdc, page, area_w, area_h, req.fit_to_page) {
            let _ = AbortDoc(hdc);
            return Err(e);
        }
    }
    if EndDoc(hdc) <= 0 {
        return Err(PrintError::printer(
            "Druckauftrag konnte nicht abgeschlossen werden".to_string(),
        ));
    }
    Ok(())
}

impl PrintBackend for GdiBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed: u32 = 0;
            let mut returned: u32 = 0;
            // Sizing call: expected to fail with ERROR_INSUFFICIENT_BUFFER.
            let _ = EnumPrintersW(flags, None, 4, None, &mut needed, &mut returned);
            if needed == 0 {
                return Ok(Vec::new());
            }

            let mut buf = AlignedBuffer::new(needed as usize);
            EnumPrintersW(
                flags,
                None,
                4,
                Some(buf.as_mut_bytes()),
                &mut needed,
                &mut returned,
            )
            .map_err(|e| PrintError::config(format!("Druckerliste nicht lesbar: {e}")))?;

            let items =
                std::slice::from_raw_parts(buf.as_ptr().cast::<PRINTER_INFO_4W>(), returned as usize);
            let default = default_printer_name();
            let mut out = Vec::with_capacity(items.len());
            for p in items {
                if p.pPrinterName.is_null() {
                    continue;
                }
                let name = p.pPrinterName.to_string().unwrap_or_default();
                let is_default = default.as_deref() == Some(name.as_str());
                out.push(PrinterInfo { name, is_default });
            }
            Ok(out)
        }
    }

    fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError> {
        unsafe {
            let name = wide(printer);
            Ok(PrinterCapabilities {
                duplex: device_cap(&name, DC_DUPLEX) > 0,
                color: device_cap(&name, DC_COLORDEVICE) > 0,
                copies: device_cap(&name, DC_COPIES) > 1,
            })
        }
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if !req.file.exists() {
            return Err(PrintError::file(format!(
                "Datei nicht gefunden: {}",
                req.file.display()
            )));
        }
        unsafe {
            let (hdc, device_dpi) = open_dc(req)?;
            // Every early return past CreateDCW must still delete the DC.
            let result = print_on_dc(hdc, device_dpi, req, self.pdfium_path.as_deref());
            let _ = DeleteDC(hdc);
            result
        }
    }
}
