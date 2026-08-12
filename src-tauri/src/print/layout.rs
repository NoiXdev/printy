/// Rasterising above this DPI buys nothing and costs a lot of memory:
/// an A4 page at 300 dpi is roughly 26 MB as 24-bit RGB.
pub const MAX_RENDER_DPI: i32 = 300;
pub const MIN_RENDER_DPI: i32 = 72;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Scales `src` to fit inside `area` preserving aspect ratio, centered.
pub fn fit_centered(src_w: i32, src_h: i32, area_w: i32, area_h: i32) -> FitRect {
    if src_w <= 0 || src_h <= 0 || area_w <= 0 || area_h <= 0 {
        return FitRect { x: 0, y: 0, width: 0, height: 0 };
    }
    let scale = f64::min(area_w as f64 / src_w as f64, area_h as f64 / src_h as f64);
    let width = (src_w as f64 * scale).round() as i32;
    let height = (src_h as f64 * scale).round() as i32;
    FitRect {
        x: (area_w - width) / 2,
        y: (area_h - height) / 2,
        width,
        height,
    }
}

/// Clamps a printer's reported DPI into the range we are willing to rasterise.
pub fn render_dpi(device_dpi: i32) -> i32 {
    device_dpi.clamp(MIN_RENDER_DPI, MAX_RENDER_DPI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_wider_than_area_is_letterboxed_vertically() {
        // 200x100 source into a 400x400 area -> scaled to 400x200, centered.
        let r = fit_centered(200, 100, 400, 400);
        assert_eq!((r.width, r.height), (400, 200));
        assert_eq!((r.x, r.y), (0, 100));
    }

    #[test]
    fn image_taller_than_area_is_pillarboxed_horizontally() {
        let r = fit_centered(100, 200, 400, 400);
        assert_eq!((r.width, r.height), (200, 400));
        assert_eq!((r.x, r.y), (100, 0));
    }

    #[test]
    fn exact_aspect_match_fills_the_area() {
        let r = fit_centered(210, 297, 2100, 2970);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 2100, 2970));
    }

    #[test]
    fn degenerate_sizes_do_not_panic_or_divide_by_zero() {
        let r = fit_centered(0, 0, 400, 400);
        assert_eq!((r.width, r.height), (0, 0));
    }

    #[test]
    fn render_dpi_is_capped_to_bound_memory() {
        assert_eq!(render_dpi(600), MAX_RENDER_DPI);
        assert_eq!(render_dpi(200), 200);
        assert_eq!(render_dpi(0), MIN_RENDER_DPI);
    }

    #[test]
    fn source_larger_than_area_is_scaled_down() {
        // 2000x1000 source into a 400x400 area
        // scale = min(400/2000, 400/1000) = min(0.2, 0.4) = 0.2
        // width = round(2000 * 0.2) = 400
        // height = round(1000 * 0.2) = 200
        // x = (400 - 400) / 2 = 0
        // y = (400 - 200) / 2 = 100
        let r = fit_centered(2000, 1000, 400, 400);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 100, 400, 200));
        assert!(r.width <= 400);
        assert!(r.height <= 400);
    }

    #[test]
    fn mixed_ratio_downscales_by_the_limiting_dimension() {
        // 800x100 source into a 400x400 area
        // scale = min(400/800, 400/100) = min(0.5, 4.0) = 0.5
        // width = round(800 * 0.5) = 400
        // height = round(100 * 0.5) = 50
        // x = (400 - 400) / 2 = 0
        // y = (400 - 50) / 2 = 175
        let r = fit_centered(800, 100, 400, 400);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 175, 400, 50));
        assert!(r.width <= 400);
        assert!(r.height <= 400);
    }

    #[test]
    fn non_positive_area_yields_a_zero_rect() {
        // Zero width area
        let r = fit_centered(1000, 1000, 0, 400);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 0, 0));

        // Zero height area
        let r = fit_centered(1000, 1000, 400, 0);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 0, 0));

        // Negative area dimension
        let r = fit_centered(1000, 1000, -100, 400);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 0, 0));
    }
}
