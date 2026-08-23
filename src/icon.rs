use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use icns::{IconFamily, Image, PixelFormat};
use image::RgbaImage;
use rand::Rng;

use crate::image_ops;

pub fn write(source: &RgbaImage, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("icon destination has no parent directory")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let nonce: u32 = rand::rng().random();
    let temporary = parent.join(format!(".Ghostty.{}.{}.icns", std::process::id(), nonce));
    let result = write_temporary(source, &temporary).and_then(|()| {
        std::fs::rename(&temporary, destination)
            .with_context(|| format!("installing {}", destination.display()))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn write_temporary(source: &RgbaImage, path: &Path) -> Result<()> {
    let mut family = IconFamily::new();
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let rendered = image_ops::render_square(source, size, 7);
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
    writer.flush().context("flushing generated icon")
}
