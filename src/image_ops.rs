use gtk4::cairo;
use gtk4::pango;

use image::{DynamicImage, imageops::FilterType};
use std::path::Path;

// ── Text annotation ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TextAnnotation {
    /// Position in image-pixel coordinates (top-left corner of the text).
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub font_desc: pango::FontDescription,
    /// RGBA components in 0.0 – 1.0.
    pub color: (f64, f64, f64, f64),
    /// Rotation in radians, clockwise.
    pub rotation: f64,
    /// Rotation pivot offset from (x, y) in image-pixel coordinates.
    pub pivot_dx: f64,
    pub pivot_dy: f64,
}

/// Draw a single annotation onto the given Cairo context.
/// The context must already be in image-space (scaled/translated by the caller).
pub fn draw_text_annotation(cr: &cairo::Context, ann: &TextAnnotation) {
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_font_description(Some(&ann.font_desc));
    layout.set_text(&ann.text);
    cr.set_source_rgba(ann.color.0, ann.color.1, ann.color.2, ann.color.3);
    cr.save().unwrap();
    // Translate to pivot point, rotate, then draw text offset from pivot
    cr.translate(ann.x + ann.pivot_dx, ann.y + ann.pivot_dy);
    cr.rotate(ann.rotation);
    cr.move_to(-ann.pivot_dx, -ann.pivot_dy);
    pangocairo::functions::show_layout(cr, &layout);
    cr.restore().unwrap();
}

/// Flatten all annotations onto `img`, returning a new `DynamicImage`.
pub fn flatten_annotations(img: &DynamicImage, annotations: &[TextAnnotation]) -> DynamicImage {
    if annotations.is_empty() {
        return img.clone();
    }
    let mut surface = to_cairo_surface(img);
    {
        let cr = cairo::Context::new(&surface).expect("cairo context");
        for ann in annotations {
            draw_text_annotation(&cr, ann);
        }
    } // cr dropped — all drawing complete before we read back pixel data
    surface_to_image(&mut surface).unwrap_or_else(|| img.clone())
}

/// Convert a Cairo ARgb32 surface back to a `DynamicImage` (un-premultiplies alpha).
pub fn surface_to_image(surface: &mut cairo::ImageSurface) -> Option<DynamicImage> {
    let w = surface.width() as u32;
    let h = surface.height() as u32;
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        for col in 0..w as usize {
            let off = row * stride + col * 4;
            let b = data[off] as u32;
            let g = data[off + 1] as u32;
            let r = data[off + 2] as u32;
            let a = data[off + 3] as u32;
            if a == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.push(((r * 255 + a / 2) / a).min(255) as u8);
                rgba.push(((g * 255 + a / 2) / a).min(255) as u8);
                rgba.push(((b * 255 + a / 2) / a).min(255) as u8);
                rgba.push(a as u8);
            }
        }
    }
    Some(DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, rgba)?))
}

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
