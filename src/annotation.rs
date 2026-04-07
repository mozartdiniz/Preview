use gtk4::cairo;
use gtk4::gdk;
use gtk4::pango;
use gtk4::prelude::{TextureExt, TextureExtManual};

use image::DynamicImage;

// ── Shape annotations ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ShapeKind { Rect, Line, Arrow }

impl Default for ShapeKind {
    fn default() -> Self { ShapeKind::Rect }
}

#[derive(Clone)]
pub struct ShapeAnnotation {
    pub kind: ShapeKind,
    /// Image-pixel coordinates of the two defining points.
    pub x1: f64, pub y1: f64,
    pub x2: f64, pub y2: f64,
    pub color: (f64, f64, f64, f64),
    /// Stroke width in image pixels.
    pub stroke_width: f64,
}

pub fn draw_shape_annotation(cr: &cairo::Context, ann: &ShapeAnnotation) {
    let (r, g, b, a) = ann.color;
    cr.set_source_rgba(r, g, b, a);
    cr.set_line_width(ann.stroke_width);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    match ann.kind {
        ShapeKind::Rect => {
            let (x1, y1) = (ann.x1.min(ann.x2), ann.y1.min(ann.y2));
            let (x2, y2) = (ann.x1.max(ann.x2), ann.y1.max(ann.y2));
            cr.rectangle(x1, y1, x2 - x1, y2 - y1);
            cr.stroke().unwrap();
        }
        ShapeKind::Line => {
            cr.move_to(ann.x1, ann.y1);
            cr.line_to(ann.x2, ann.y2);
            cr.stroke().unwrap();
        }
        ShapeKind::Arrow => {
            let dx = ann.x2 - ann.x1;
            let dy = ann.y2 - ann.y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1.0 { return; }
            let ux = dx / len;
            let uy = dy / len;
            let head = (ann.stroke_width * 5.0 + 10.0).min(len);
            // Shaft stops short of tip so it doesn't poke through the filled head
            cr.move_to(ann.x1, ann.y1);
            cr.line_to(ann.x2 - ux * head * 0.65, ann.y2 - uy * head * 0.65);
            cr.stroke().unwrap();
            // Filled arrowhead triangle
            cr.move_to(ann.x2, ann.y2);
            cr.line_to(ann.x2 - ux * head + uy * head * 0.38, ann.y2 - uy * head - ux * head * 0.38);
            cr.line_to(ann.x2 - ux * head - uy * head * 0.38, ann.y2 - uy * head + ux * head * 0.38);
            cr.close_path();
            cr.fill().unwrap();
        }
    }
}

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
}

/// Draw a single annotation onto the given Cairo context.
/// The context must already be in image-space (scaled/translated by the caller).
/// Rotation is always around the centre of the text bounding box.
pub fn draw_text_annotation(cr: &cairo::Context, ann: &TextAnnotation) {
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_font_description(Some(&ann.font_desc));
    layout.set_text(&ann.text);
    let (tw, th) = layout.pixel_size();
    let half_w = tw as f64 / 2.0;
    let half_h = th as f64 / 2.0;
    cr.set_source_rgba(ann.color.0, ann.color.1, ann.color.2, ann.color.3);
    cr.save().unwrap();
    cr.translate(ann.x + half_w, ann.y + half_h);
    cr.rotate(ann.rotation);
    cr.move_to(-half_w, -half_h);
    pangocairo::functions::show_layout(cr, &layout);
    cr.restore().unwrap();
}

/// Flatten all annotations onto `img`, returning a new `DynamicImage`.
pub fn flatten_annotations(
    img: &DynamicImage,
    annotations: &[TextAnnotation],
    shapes: &[ShapeAnnotation],
) -> DynamicImage {
    if annotations.is_empty() && shapes.is_empty() {
        return img.clone();
    }
    let mut surface = to_cairo_surface(img);
    {
        let cr = cairo::Context::new(&surface).expect("cairo context");
        for ann in annotations {
            draw_text_annotation(&cr, ann);
        }
        for shape in shapes {
            draw_shape_annotation(&cr, shape);
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

// ── GDK texture → Cairo surface (for display-only formats like SVG) ───────────

pub fn gdk_texture_to_cairo(texture: &gdk::Texture) -> Option<cairo::ImageSurface> {
    let w = texture.width();
    let h = texture.height();
    let stride = (w * 4) as usize;
    // download() gives RGBA straight-alpha
    let mut rgba = vec![0u8; stride * h as usize];
    texture.download(&mut rgba, stride);
    // Convert to premultiplied BGRA (cairo ARgb32 on little-endian)
    let mut bgra: Vec<u8> = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        let a = chunk[3] as u32;
        bgra.push(((b * a + 127) / 255) as u8);
        bgra.push(((g * a + 127) / 255) as u8);
        bgra.push(((r * a + 127) / 255) as u8);
        bgra.push(a as u8);
    }
    cairo::ImageSurface::create_for_data(
        bgra,
        cairo::Format::ARgb32,
        w,
        h,
        stride as i32,
    )
    .ok()
}
