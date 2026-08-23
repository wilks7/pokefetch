use std::path::Path;

use anyhow::{Context, Result};
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, Rgba, RgbaImage};

pub fn load_rgba(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("decoding {}", path.display()))?
        .to_rgba8())
}

pub fn render_square(source: &RgbaImage, size: u32, margin_percent: u32) -> RgbaImage {
    let cropped = crop_transparency(source);
    let usable =
        size.saturating_mul(100_u32.saturating_sub(margin_percent.saturating_mul(2))) / 100;
    let scale =
        (usable as f64 / cropped.width() as f64).min(usable as f64 / cropped.height() as f64);
    let width = ((cropped.width() as f64 * scale).round() as u32).max(1);
    let height = ((cropped.height() as f64 * scale).round() as u32).max(1);
    let resized = imageops::resize(&cropped, width, height, FilterType::Nearest);
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let x = i64::from((size - width) / 2);
    let y = i64::from((size - height) / 2);
    imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, PngFilterType::NoFilter)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .context("encoding PNG")?;
    Ok(bytes)
}

pub fn save_png(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("writing {}", path.display()))
}

fn crop_transparency(source: &RgbaImage) -> RgbaImage {
    let mut left = source.width();
    let mut top = source.height();
    let mut right = 0;
    let mut bottom = 0;
    let mut found = false;

    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] > 0 {
            found = true;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }

    if !found {
        return source.clone();
    }

    imageops::crop_imm(source, left, top, right - left + 1, bottom - top + 1).to_image()
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::render_square;

    #[test]
    fn centers_and_scales_without_smoothing() {
        let mut source = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        source.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        source.put_pixel(2, 1, Rgba([0, 0, 255, 255]));
        let output = render_square(&source, 8, 0);
        assert_eq!(output.dimensions(), (8, 8));
        assert!(output
            .pixels()
            .any(|pixel| *pixel == Rgba([255, 0, 0, 255])));
        assert!(output
            .pixels()
            .any(|pixel| *pixel == Rgba([0, 0, 255, 255])));
    }
}
