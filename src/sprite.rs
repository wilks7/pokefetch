use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rand::Rng;

use crate::config::{cache_dir, SpriteConfig};

#[cfg(feature = "bundle-gen1")]
mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundled.rs"));
}

#[cfg(feature = "bundle-gen1")]
pub fn bundled_palette(id: u16, variant: &str) -> Option<[crate::palette::Color; 4]> {
    if variant != "red-blue" {
        return None;
    }
    bundled::palette(id)
        .map(|colors| colors.map(|(red, green, blue)| crate::palette::Color { red, green, blue }))
}

#[cfg(not(feature = "bundle-gen1"))]
pub fn bundled_palette(_id: u16, _variant: &str) -> Option<[crate::palette::Color; 4]> {
    None
}

const SPRITE_BASE_URL: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon";

pub struct SpriteStore<'a> {
    config: &'a SpriteConfig,
    config_dir: &'a Path,
}

impl<'a> SpriteStore<'a> {
    pub fn new(config: &'a SpriteConfig, config_dir: &'a Path) -> Self {
        Self { config, config_dir }
    }

    pub fn resolve(&self, id: u16) -> Result<PathBuf> {
        let variant = self.variant();
        let local = self
            .config_dir
            .join("sprites")
            .join(&variant)
            .join(format!("{id}.png"));
        if is_populated(&local) {
            return Ok(local);
        }

        let cache = cache_dir()
            .join("sprites")
            .join(&variant)
            .join(format!("{id}.png"));
        if is_populated(&cache) {
            return Ok(cache);
        }

        #[cfg(feature = "bundle-gen1")]
        if variant == "red-blue" {
            if let Some(bytes) = bundled::sprite(id) {
                atomic_write(&cache, bytes)?;
                return Ok(cache);
            }
        }

        let primary_url = self.url(id)?;
        let bytes = match download(&primary_url) {
            Ok(bytes) => bytes,
            Err(primary_error) if variant != "default" => {
                let fallback_url = format!("{SPRITE_BASE_URL}/{id}.png");
                download(&fallback_url).with_context(|| {
                    format!(
                        "fetching {variant} sprite failed ({primary_error}); default fallback also failed"
                    )
                })?
            }
            Err(error) => return Err(error),
        };
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .context("downloaded sprite was not a valid PNG")?;
        atomic_write(&cache, &bytes)?;
        Ok(cache)
    }

    pub fn variant(&self) -> String {
        if self.config.artwork {
            "official-artwork".to_string()
        } else if self.config.variant.trim().is_empty() {
            "default".to_string()
        } else {
            self.config.variant.trim().to_string()
        }
    }

    fn url(&self, id: u16) -> Result<String> {
        if self.config.artwork {
            return Ok(format!("{SPRITE_BASE_URL}/other/official-artwork/{id}.png"));
        }

        let variant = self.config.variant.trim();
        if variant.is_empty() || variant == "default" {
            return Ok(format!("{SPRITE_BASE_URL}/{id}.png"));
        }
        let Some(generation) = generation_for(variant) else {
            bail!(
                "no PokeAPI generation mapping for sprite variant {variant:?}; add a local sprite or choose a known variant"
            );
        };
        Ok(format!(
            "{SPRITE_BASE_URL}/versions/{generation}/{variant}/transparent/{id}.png"
        ))
    }
}

fn generation_for(variant: &str) -> Option<&'static str> {
    match variant {
        "red-blue" | "yellow" => Some("generation-i"),
        "gold" | "silver" | "crystal" => Some("generation-ii"),
        "ruby-sapphire" | "emerald" | "firered-leafgreen" => Some("generation-iii"),
        "diamond-pearl" | "platinum" | "heartgold-soulsilver" => Some("generation-iv"),
        "black-white" => Some("generation-v"),
        "omegaruby-alphasapphire" | "x-y" => Some("generation-vi"),
        "ultra-sun-ultra-moon" | "icons" => Some("generation-vii"),
        "icons8" => Some("generation-viii"),
        "scarlet-violet" => Some("generation-ix"),
        _ => None,
    }
}

fn download(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("User-Agent", "pokefetch/0.1")
        .call()
        .with_context(|| format!("downloading {url}"))?;
    response
        .body_mut()
        .read_to_vec()
        .with_context(|| format!("reading {url}"))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("sprite cache path has no parent directory")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let nonce: u32 = rand::rng().random();
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sprite"),
        std::process::id(),
        nonce
    ));
    std::fs::write(&temporary, bytes)
        .with_context(|| format!("writing {}", temporary.display()))?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_error) if is_populated(path) => {
            let _ = std::fs::remove_file(&temporary);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error).with_context(|| format!("installing {}", path.display()))
        }
    }
}

fn is_populated(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| metadata.len() > 0)
}

#[cfg(test)]
mod tests {
    use super::generation_for;

    #[test]
    fn maps_the_checked_in_sprite_variant() {
        assert_eq!(generation_for("red-blue"), Some("generation-i"));
        assert_eq!(generation_for("crystal"), Some("generation-ii"));
        assert_eq!(generation_for("made-up"), None);
    }
}
