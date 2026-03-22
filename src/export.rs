use image::DynamicImage;
use std::path::Path;

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
