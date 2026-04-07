use gtk4::pango;

use crate::annotation::{ShapeAnnotation, ShapeKind, TextAnnotation};

// ── Shape hit testing ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ShapeHandle { P1, P2 }

fn point_to_segment_dist(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1; let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-10 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * dx + (py - y1) * dy) / len2;
    let qx = x1 + t.clamp(0.0, 1.0) * dx;
    let qy = y1 + t.clamp(0.0, 1.0) * dy;
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// Hit-test `(ix, iy)` (image-space coords) against shape outlines.
/// `scale` converts widget pixels to image pixels for the tolerance calculation.
/// Returns the topmost (last-drawn) hit index.
pub fn hit_test_shape(
    shapes: &[ShapeAnnotation],
    ix: f64,
    iy: f64,
    scale: f64,
) -> Option<usize> {
    for (i, shape) in shapes.iter().enumerate().rev() {
        let pad = shape.stroke_width / 2.0 + 8.0 / scale;
        let hit = match shape.kind {
            ShapeKind::Rect => {
                let (lx, rx) = (shape.x1.min(shape.x2), shape.x1.max(shape.x2));
                let (ty, by) = (shape.y1.min(shape.y2), shape.y1.max(shape.y2));
                let d = point_to_segment_dist(ix, iy, lx, ty, rx, ty)
                    .min(point_to_segment_dist(ix, iy, lx, by, rx, by))
                    .min(point_to_segment_dist(ix, iy, lx, ty, lx, by))
                    .min(point_to_segment_dist(ix, iy, rx, ty, rx, by));
                d <= pad
            }
            ShapeKind::Line | ShapeKind::Arrow => {
                point_to_segment_dist(ix, iy, shape.x1, shape.y1, shape.x2, shape.y2) <= pad
            }
        };
        if hit { return Some(i); }
    }
    None
}

/// Hit-test the P1 or P2 endpoint handle of a shape.
/// Handle radius is 10 widget pixels, converted to image space via `scale`.
pub fn hit_test_shape_handle(
    shape: &ShapeAnnotation,
    ix: f64,
    iy: f64,
    scale: f64,
) -> Option<ShapeHandle> {
    let r = 10.0 / scale;
    if ((ix - shape.x1).powi(2) + (iy - shape.y1).powi(2)).sqrt() <= r {
        return Some(ShapeHandle::P1);
    }
    if ((ix - shape.x2).powi(2) + (iy - shape.y2).powi(2)).sqrt() <= r {
        return Some(ShapeHandle::P2);
    }
    None
}

// ── Hit testing ───────────────────────────────────────────────────────────────

/// Returns the index of the topmost annotation whose text bounding box contains (wx, wy)
/// in widget (viewport) coordinates.
pub fn hit_test_annotation(
    annotations: &[TextAnnotation],
    wx: f64,
    wy: f64,
    ox: f64,
    oy: f64,
    scale: f64,
    pango_ctx: &pango::Context,
) -> Option<usize> {
    for (i, ann) in annotations.iter().enumerate().rev() {
        let layout = pango::Layout::new(pango_ctx);
        layout.set_font_description(Some(&ann.font_desc));
        layout.set_text(&ann.text);
        let (tw, th) = layout.pixel_size();
        let cx = ox + ann.x * scale + tw as f64 / 2.0;
        let cy = oy + ann.y * scale + th as f64 / 2.0;
        let dx = (wx - cx) / scale;
        let dy = (wy - cy) / scale;
        let cos_r = ann.rotation.cos();
        let sin_r = ann.rotation.sin();
        let lx = dx * cos_r + dy * sin_r;
        let ly = -dx * sin_r + dy * cos_r;
        let pad = 4.0 / scale;
        let half_w = tw as f64 / scale / 2.0 + pad;
        let half_h = th as f64 / scale / 2.0 + pad;
        if lx >= -half_w && lx <= half_w && ly >= -half_h && ly <= half_h {
            return Some(i);
        }
    }
    None
}

/// Returns true if (wx, wy) is on a corner rotation handle of `ann`.
pub fn hit_test_rotation_handle(
    ann: &TextAnnotation,
    wx: f64,
    wy: f64,
    ox: f64,
    oy: f64,
    scale: f64,
    pango_ctx: &pango::Context,
) -> bool {
    let layout = pango::Layout::new(pango_ctx);
    layout.set_font_description(Some(&ann.font_desc));
    layout.set_text(&ann.text);
    let (tw, th) = layout.pixel_size();
    let cx = ox + ann.x * scale + tw as f64 / 2.0;
    let cy = oy + ann.y * scale + th as f64 / 2.0;
    let dx = (wx - cx) / scale;
    let dy = (wy - cy) / scale;
    let cos_r = ann.rotation.cos();
    let sin_r = ann.rotation.sin();
    let lx = dx * cos_r + dy * sin_r;
    let ly = -dx * sin_r + dy * cos_r;
    let pad = 4.0 / scale;
    let hit_r = 10.0 / scale;
    let half_w = tw as f64 / scale / 2.0 + pad;
    let half_h = th as f64 / scale / 2.0 + pad;
    for (hx, hy) in [(-half_w, -half_h), (half_w, -half_h), (-half_w, half_h), (half_w, half_h)] {
        if ((lx - hx).powi(2) + (ly - hy).powi(2)).sqrt() <= hit_r {
            return true;
        }
    }
    false
}
