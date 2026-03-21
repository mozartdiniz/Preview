use gtk4::cairo;
use image::{DynamicImage, imageops::FilterType};
use std::path::Path;

// ── Display conversion ────────────────────────────────────────────────────────

/// Convert a DynamicImage to a Cairo ImageSurface (premultiplied BGRA).
/// This is the surface used for rendering in the DrawingArea draw func.
pub fn to_cairo_surface(img: &DynamicImage) -> cairo::ImageSurface {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let stride = (w * 4) as usize;

    // Cairo Argb32 = premultiplied ARGB stored as BGRA on little-endian x86.
    let mut data: Vec<u8> = Vec::with_capacity(stride * h as usize);
    for p in rgba.pixels() {
        let r = p[0] as u32;
        let g = p[1] as u32;
        let b = p[2] as u32;
        let a = p[3] as u32;
        data.push(((b * a + 127) / 255) as u8);
        data.push(((g * a + 127) / 255) as u8);
        data.push(((r * a + 127) / 255) as u8);
        data.push(a as u8);
    }

    cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        w as i32,
        h as i32,
        stride as i32,
    )
    .expect("cairo surface creation failed")
}

// ── Resize ────────────────────────────────────────────────────────────────────

pub fn resize(img: &DynamicImage, w: u32, h: u32) -> DynamicImage {
    img.resize_exact(w, h, FilterType::Lanczos3)
}

// ── Rotate ────────────────────────────────────────────────────────────────────

pub fn rotate_cw(img: &DynamicImage) -> DynamicImage {
    img.rotate90()
}

pub fn rotate_ccw(img: &DynamicImage) -> DynamicImage {
    img.rotate270()
}

// ── Flip ─────────────────────────────────────────────────────────────────────

pub fn flip_h(img: &DynamicImage) -> DynamicImage {
    img.fliph()
}

pub fn flip_v(img: &DynamicImage) -> DynamicImage {
    img.flipv()
}

// ── Crop ─────────────────────────────────────────────────────────────────────

pub fn crop(img: &DynamicImage, x: u32, y: u32, w: u32, h: u32) -> Option<DynamicImage> {
    let iw = img.width();
    let ih = img.height();
    let x = x.min(iw.saturating_sub(1));
    let y = y.min(ih.saturating_sub(1));
    let w = w.min(iw - x);
    let h = h.min(ih - y);
    if w == 0 || h == 0 {
        return None;
    }
    Some(img.crop_imm(x, y, w, h))
}

// ── Save / Export ─────────────────────────────────────────────────────────────

pub fn save_image(img: &DynamicImage, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => {
            use image::codecs::jpeg::JpegEncoder;
            let file = std::fs::File::create(path)?;
            let encoder = JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 92);
            img.write_with_encoder(encoder)?;
        }
        _ => {
            img.save(path)?;
        }
    }
    Ok(())
}

// ── Coordinate helpers ────────────────────────────────────────────────────────

/// Returns (offset_x, offset_y, scale) for contain-fit of img inside viewport.
pub fn fit_transform(img_w: i32, img_h: i32, vp_w: f64, vp_h: f64) -> (f64, f64, f64) {
    let iw = img_w as f64;
    let ih = img_h as f64;
    let scale = (vp_w / iw).min(vp_h / ih);
    let ox = (vp_w - iw * scale) / 2.0;
    let oy = (vp_h - ih * scale) / 2.0;
    (ox, oy, scale)
}

/// Widget coordinate → image pixel coordinate (clamped).
pub fn widget_to_img(
    wx: f64,
    wy: f64,
    img_w: i32,
    img_h: i32,
    ox: f64,
    oy: f64,
    scale: f64,
) -> (u32, u32) {
    let ix = ((wx - ox) / scale).round().clamp(0.0, img_w as f64) as u32;
    let iy = ((wy - oy) / scale).round().clamp(0.0, img_h as f64) as u32;
    (ix, iy)
}
