//! Writing a macOS `.icns` file from a sprite.
//!
//! Ghostty reads its dock icon from disk at launch, so Pokefetch writes one
//! during a greeting and the *next* Ghostty window picks it up. The icon
//! therefore always trails the greeting by one launch — that is expected, not
//! a bug.
//!
//! An `.icns` is a container holding the same artwork at several resolutions;
//! macOS picks whichever fits the context. All seven are generated from the
//! same sprite by nearest-neighbor scaling.
//!
//! # Rust concepts on display
//!
//! - **Cleanup on the error path**: [`write()`] removes its temporary file when
//!   anything fails. Rust has no `finally`, so the result is captured, cleaned
//!   up after, and only then returned.
//! - **`BufWriter`**: wrapping a [`File`] batches many small writes into few
//!   syscalls. Writing an icon family is exactly that access pattern.
//! - **`.and_then` on `Result`**: chaining a second fallible step onto a first,
//!   short-circuiting if the first failed.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use icns::{IconFamily, Image, PixelFormat};
use image::RgbaImage;
use rand::Rng;

use crate::image_ops;

/// Icon resolutions macOS expects in a complete `.icns`.
const ICON_SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

/// Border reserved on each side, as a percentage.
///
/// Dock icons look wrong flush against their bounds; macOS artwork
/// conventionally leaves a margin.
const ICON_MARGIN_PERCENT: u32 = 7;

/// Writes `source` to `destination` as a multi-resolution `.icns`.
///
/// The write is atomic: a temporary file is built first and renamed into place,
/// so Ghostty can never observe a partially written icon at launch. That
/// matters because this runs in a background process while the user is
/// actively opening terminals.
///
/// # Errors
///
/// Returns an error if the destination directory cannot be created, if any
/// resolution fails to encode, or if the final rename fails.
pub fn write(source: &RgbaImage, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("icon destination has no parent directory")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    // Same directory as the destination, because rename is only atomic within
    // one filesystem. Process id plus a nonce keeps concurrent runs apart.
    let nonce: u32 = rand::rng().random();
    let temporary = parent.join(format!(".Ghostty.{}.{}.icns", std::process::id(), nonce));
    let result = write_temporary(source, &temporary).and_then(|()| {
        std::fs::rename(&temporary, destination)
            .with_context(|| format!("installing {}", destination.display()))
    });
    // Rust has no `finally`, so cleanup happens here, after the result is in
    // hand but before it is returned.
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Builds every icon resolution and writes them to one file.
fn write_temporary(source: &RgbaImage, path: &Path) -> Result<()> {
    let mut family = IconFamily::new();
    for size in ICON_SIZES {
        let rendered = image_ops::render_square(source, size, ICON_MARGIN_PERCENT);
        // `into_raw` hands ownership of the pixel buffer to the icns crate
        // rather than copying it.
        let icon = Image::from_data(PixelFormat::RGBA, size, size, rendered.into_raw())
            .with_context(|| format!("building {size}x{size} icon representation"))?;
        family
            .add_icon(&icon)
            .with_context(|| format!("adding {size}x{size} icon representation"))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    family
        .write(&mut writer)
        .with_context(|| format!("encoding {}", path.display()))?;
    // A BufWriter can still be holding bytes when it is dropped, and drop
    // cannot report an error. Flushing explicitly surfaces a failed write.
    writer.flush().context("flushing generated icon")
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{write, ICON_SIZES};

    #[test]
    fn writes_an_icns_container_and_leaves_no_temporary() {
        let directory = std::env::temp_dir().join(format!("pokefetch-icon-{}", std::process::id()));
        let destination = directory.join("Test.icns");
        let source = RgbaImage::from_pixel(8, 8, Rgba([255, 128, 0, 255]));

        write(&source, &destination).unwrap();
        assert!(destination.is_file());

        let bytes = std::fs::read(&destination).unwrap();
        assert_eq!(&bytes[..4], b"icns", "icns files start with a magic header");
        assert!(bytes.len() > ICON_SIZES.len() * 4, "every size is present");

        let leftovers = std::fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
