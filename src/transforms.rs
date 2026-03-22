use image::{DynamicImage, imageops::FilterType};

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
