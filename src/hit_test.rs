use gtk4::pango;

use crate::annotation::TextAnnotation;

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
