//! Decoding, cropping, scaling, and encoding sprite pixels and animations.
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

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{ensure, Context, Result};
use image::codecs::gif::GifDecoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{self, FilterType};
use image::{
    AnimationDecoder, ExtendedColorType, Frame, ImageEncoder, ImageFormat, Rgba, RgbaImage,
};

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

/// Loads every composited frame from an animated GIF.
///
/// The decoder applies GIF disposal rules, so each returned frame is a full
/// canvas rather than a delta that depends on the preceding frame.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, is not a decodable GIF, or
/// contains no frames.
pub fn load_gif_frames(path: &Path) -> Result<Vec<Frame>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = GifDecoder::new(BufReader::new(file))
        .with_context(|| format!("decoding {}", path.display()))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .with_context(|| format!("decoding frames from {}", path.display()))?;
    ensure!(
        !frames.is_empty(),
        "{} contains no GIF frames",
        path.display()
    );
    Ok(frames)
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
    fit_square(&cropped, size, margin_percent)
}

/// Fits animation frames into equal transparent square canvases.
///
/// A single transparency bound is shared across every frame. Cropping each
/// frame independently would make a moving limb resize or jump because its
/// changing outline would produce a different scale and center.
///
/// # Errors
///
/// Returns an error if `frames` is empty or its frames do not share one canvas
/// size.
pub fn render_animation_square(
    frames: &[Frame],
    size: u32,
    margin_percent: u32,
) -> Result<Vec<Frame>> {
    let first = frames.first().context("animation contains no frames")?;
    let dimensions = first.buffer().dimensions();
    ensure!(
        frames
            .iter()
            .all(|frame| frame.buffer().dimensions() == dimensions),
        "animation frames must share one canvas size"
    );
    let bounds = transparency_bounds(frames.iter().map(Frame::buffer))
        .unwrap_or_else(|| Bounds::entire(dimensions.0, dimensions.1));

    Ok(frames
        .iter()
        .map(|frame| {
            Frame::from_parts(
                render_square_with_bounds(frame.buffer(), bounds, size, margin_percent),
                0,
                0,
                frame.delay(),
            )
        })
        .collect())
}

/// Fits one image into a square using a transparency bound chosen by its
/// caller.
fn render_square_with_bounds(
    source: &RgbaImage,
    bounds: Bounds,
    size: u32,
    margin_percent: u32,
) -> RgbaImage {
    let cropped =
        imageops::crop_imm(source, bounds.left, bounds.top, bounds.width, bounds.height).to_image();
    fit_square(&cropped, size, margin_percent)
}

/// Scales and centers pixels that have already been cropped consistently.
fn fit_square(cropped: &RgbaImage, size: u32, margin_percent: u32) -> RgbaImage {
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

    let resized = imageops::resize(cropped, width, height, FilterType::Nearest);
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

/// The inclusive content rectangle shared by one or more images.
#[derive(Clone, Copy)]
struct Bounds {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl Bounds {
    /// Covers a complete image canvas.
    const fn entire(width: u32, height: u32) -> Self {
        Self {
            left: 0,
            top: 0,
            width,
            height,
        }
    }
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
    encode_png_with_filter(image, PngFilterType::NoFilter)
}

/// Encodes one animation frame as a compact PNG in memory.
///
/// Animation multiplies transfer size by its frame count, so adaptive row
/// filtering is worth its small CPU cost there. Still greetings continue to
/// use [`encode_png`]'s lower-latency, unfiltered path.
///
/// # Errors
///
/// Returns an error if encoding fails.
pub fn encode_animation_frame_png(image: &RgbaImage) -> Result<Vec<u8>> {
    encode_png_with_filter(image, PngFilterType::Adaptive)
}

/// Encodes RGBA pixels with the selected PNG row filter.
fn encode_png_with_filter(image: &RgbaImage, filter: PngFilterType) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, filter)
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
    transparency_bounds(std::iter::once(source)).map_or_else(
        || source.clone(),
        |bounds| {
            imageops::crop_imm(source, bounds.left, bounds.top, bounds.width, bounds.height)
                .to_image()
        },
    )
}

/// Finds the union of every non-transparent pixel across a set of images.
fn transparency_bounds<'a>(images: impl IntoIterator<Item = &'a RgbaImage>) -> Option<Bounds> {
    // Start each bound at the opposite extreme so the first opaque pixel
    // replaces it. `found` distinguishes "no opaque pixels" from a real box.
    let mut left = u32::MAX;
    let mut top = u32::MAX;
    let mut right = 0;
    let mut bottom = 0;
    let mut found = false;

    for image in images {
        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel[3] > 0 {
                found = true;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
    }

    if found {
        Some(Bounds {
            left,
            top,
            // Bounds are inclusive, so each length is the difference plus one.
            width: right - left + 1,
            height: bottom - top + 1,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use image::{Delay, Frame, Rgba, RgbaImage};

    use super::{
        crop_transparency, encode_animation_frame_png, encode_png, load_gif_frames,
        render_animation_square, render_square,
    };

    #[test]
    fn decodes_the_checked_in_crystal_animation() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/sets/crystal/front-animated/249.gif");

        let frames = load_gif_frames(&path).unwrap();

        assert!(frames.len() > 1);
        assert!(frames
            .iter()
            .all(|frame| frame.buffer().dimensions() == (56, 56)));
        assert!(frames
            .iter()
            .all(|frame| frame.delay().numer_denom_ms().0 > 0));
    }

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
    fn animation_frames_share_one_crop_and_scale() {
        let mut first = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        first.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut second = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        second.put_pixel(2, 2, Rgba([0, 0, 255, 255]));
        let delay = Delay::from_numer_denom_ms(80, 1);
        let frames = vec![
            Frame::from_parts(first, 0, 0, delay),
            Frame::from_parts(second, 0, 0, delay),
        ];

        let rendered = render_animation_square(&frames, 8, 0).unwrap();

        assert_eq!(rendered[0].buffer().get_pixel(1, 1)[3], 255);
        assert_eq!(rendered[0].buffer().get_pixel(6, 6)[3], 0);
        assert_eq!(rendered[1].buffer().get_pixel(1, 1)[3], 0);
        assert_eq!(rendered[1].buffer().get_pixel(6, 6)[3], 255);
        assert_eq!(rendered[0].delay().numer_denom_ms(), (80, 1));
    }

    #[test]
    fn encodes_a_png_with_the_expected_magic_bytes() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let bytes = encode_png(&image).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let compact = encode_animation_frame_png(&image).unwrap();
        assert_eq!(&compact[..8], b"\x89PNG\r\n\x1a\n");
    }
}
