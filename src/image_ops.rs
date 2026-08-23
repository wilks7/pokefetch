//! Cropping, scaling, and encoding sprite pixels.
//!
//! The one rule this module exists to enforce: **scale with nearest-neighbor**.
//! Pixel art has hard edges on purpose, and every smoothing filter destroys
//! them. A bilinear-scaled Pikachu is a blurry Pikachu.
//!
//! ```text
//!   nearest            bilinear
//!   ██  ██             ▓▒  ▒▓
//!   ██████     vs      ▓████▓     <- edges smeared into the background
//!   ██  ██             ▓▒  ▒▓
//! ```
//!
//! # Rust concepts on display
//!
//! - **Working with an external crate's types**: [`RgbaImage`] comes from
//!   `image`. Rust's orphan rule means you cannot add inherent methods to a
//!   foreign type, so these are free functions taking `&RgbaImage`.
//! - **Deliberate `as` casts**: the geometry here genuinely needs float math,
//!   and converting back to integers can truncate. Rather than hide that, the
//!   conversion is isolated in [`scaled_length`] with the clamp that makes it
//!   safe, and the lint is silenced *there* rather than crate-wide.
//! - **`&[u8]` vs `Vec<u8>`**: [`encode_png`] returns an owned `Vec` because it
//!   creates the bytes; functions that only read take a borrowed slice.

use std::path::Path;

use anyhow::{Context, Result};
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, Rgba, RgbaImage};

/// Fully transparent, used to fill the area around a scaled sprite.
const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);

/// Loads an image from disk as RGBA pixels.
///
/// # Errors
///
/// Returns an error if the file is missing or not a decodable image.
pub fn load_rgba(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("decoding {}", path.display()))?
        .to_rgba8())
}

/// Fits a sprite into a transparent square canvas of `size` pixels.
///
/// Three steps: crop away the transparent border, scale the remainder to fit
/// while preserving aspect ratio, then center it. `margin_percent` reserves a
/// border on each side — icons want breathing room, terminal sprites less so.
///
/// Cropping first is what makes sizing consistent: upstream sprites are padded
/// to differing canvas sizes, so without it a Diglett would render tiny beside
/// an Onix purely because of empty space in the source file.
///
/// ```
/// # use pokefetch::image_ops::render_square;
/// # use image::{Rgba, RgbaImage};
/// let mut sprite = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
/// sprite.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
/// assert_eq!(render_square(&sprite, 64, 0).dimensions(), (64, 64));
/// ```
pub fn render_square(source: &RgbaImage, size: u32, margin_percent: u32) -> RgbaImage {
    let cropped = crop_transparency(source);
    // Saturating arithmetic keeps a nonsensical margin (say, 60%) from
    // underflowing to an enormous usable area.
    let usable =
        size.saturating_mul(100_u32.saturating_sub(margin_percent.saturating_mul(2))) / 100;
    // Take the smaller of the two ratios so the whole sprite fits inside the
    // box rather than overflowing on its longer axis.
    let scale = (f64::from(usable) / f64::from(cropped.width()))
        .min(f64::from(usable) / f64::from(cropped.height()));
    let width = scaled_length(cropped.width(), scale);
    let height = scaled_length(cropped.height(), scale);

    let resized = imageops::resize(&cropped, width, height, FilterType::Nearest);
    let mut canvas = RgbaImage::from_pixel(size, size, TRANSPARENT);
    // Integer division floors, so an odd remainder biases one pixel up-left.
    let x = i64::from((size - width) / 2);
    let y = i64::from((size - height) / 2);
    imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

/// Scales one dimension, clamping into a valid image length.
///
/// The clamp is what makes the cast below sound: the value is already forced
/// into `1..=u32::MAX` as a float, so truncation cannot produce a surprise.
/// A zero-length image would panic inside `image`, hence the lower bound.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn scaled_length(value: u32, scale: f64) -> u32 {
    (f64::from(value) * scale)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

/// Encodes an image as a PNG in memory.
///
/// Tuned for speed, not size: this runs on the shell-startup path, and the
/// bytes go straight into an escape sequence rather than to disk. `NoFilter`
/// with fast compression costs a few kilobytes and saves milliseconds.
///
/// # Errors
///
/// Returns an error if encoding fails.
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

/// Writes an image to disk as a PNG, creating parent directories.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or the file written.
pub fn save_png(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("writing {}", path.display()))
}

/// Trims fully transparent rows and columns from every edge.
///
/// One pass records the bounding box of anything visible, then a single crop
/// applies it. A fully transparent image has no bounding box, so it is
/// returned unchanged rather than cropped to nothing.
fn crop_transparency(source: &RgbaImage) -> RgbaImage {
    // Start each bound at the opposite extreme so the first opaque pixel
    // replaces it. `found` distinguishes "no opaque pixels" from a real box.
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

    // Bounds are inclusive, so the width is the difference plus one.
    imageops::crop_imm(source, left, top, right - left + 1, bottom - top + 1).to_image()
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{crop_transparency, encode_png, render_square};

    #[test]
    fn centers_and_scales_without_smoothing() {
        let mut source = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        source.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        source.put_pixel(2, 1, Rgba([0, 0, 255, 255]));
        let output = render_square(&source, 8, 0);
        assert_eq!(output.dimensions(), (8, 8));
        // Nearest-neighbor preserves exact source colors. A smoothing filter
        // would blend these into shades that appear nowhere in the input.
        assert!(output
            .pixels()
            .any(|pixel| *pixel == Rgba([255, 0, 0, 255])));
        assert!(output
            .pixels()
            .any(|pixel| *pixel == Rgba([0, 0, 255, 255])));
    }

    #[test]
    fn crops_away_a_transparent_border() {
        let mut source = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        source.put_pixel(4, 5, Rgba([255, 0, 0, 255]));
        source.put_pixel(5, 6, Rgba([255, 0, 0, 255]));
        assert_eq!(crop_transparency(&source).dimensions(), (2, 2));
    }

    #[test]
    fn leaves_a_fully_transparent_image_alone() {
        let source = RgbaImage::from_pixel(6, 4, Rgba([0, 0, 0, 0]));
        assert_eq!(crop_transparency(&source).dimensions(), (6, 4));
    }

    #[test]
    fn never_produces_a_zero_sized_render() {
        // A single pixel scaled into a large canvas must still round up to 1.
        let mut source = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        source.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let output = render_square(&source, 16, 40);
        assert_eq!(output.dimensions(), (16, 16));
    }

    #[test]
    fn encodes_a_png_with_the_expected_magic_bytes() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let bytes = encode_png(&image).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
