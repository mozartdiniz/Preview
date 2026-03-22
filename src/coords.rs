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
