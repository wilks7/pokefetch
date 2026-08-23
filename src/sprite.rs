use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::Rng;

use crate::config::{cache_dir, SpriteConfig};

mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundled.rs"));
}

pub fn bundled_palette(
    id: u16,
    game: &str,
    variant: &str,
) -> Option<[crate::palette::Color; crate::palette::SIZE]> {
    bundled::palette(game, variant, &id.to_string())
        .map(|colors| colors.map(|(red, green, blue)| crate::palette::Color { red, green, blue }))
}

pub fn bundle_profile() -> &'static str {
    bundled::PROFILE
}

const SPRITE_BASE_URL: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/c10459b9b0129eaca5c5d9b1cac65336debb1d08/sprites/pokemon";

pub struct SpriteStore<'a> {
    config: &'a SpriteConfig,
    config_dir: &'a Path,
    game: String,
}

impl<'a> SpriteStore<'a> {
    pub fn new(
        config: &'a SpriteConfig,
        config_dir: &'a Path,
        species: Option<u16>,
    ) -> Result<Self> {
        let legacy_game = known_game(config.variant.trim());
        let configured_game = config.game.fixed().and_then(known_game);
        let variant = if config.artwork {
            "official-artwork"
        } else if legacy_game.is_some() || config.variant.trim().is_empty() {
            "front"
        } else {
            config.variant.trim()
        };
        let game = if config.game.is_pool() && legacy_game.is_none() {
            choose_bundled_game(variant, species, config.game.pool())?
        } else {
            legacy_game
                .or(configured_game)
                .unwrap_or("red-blue")
                .to_string()
        };
        Ok(Self {
            config,
            config_dir,
            game,
        })
    }

    pub fn resolve(&self, id: u16) -> Result<PathBuf> {
        let game = self.game();
        let variant = self.variant();
        let extension = extension_for(&variant);
        let local = self
            .config_dir
            .join("sprites")
            .join(game)
            .join(&variant)
            .join(format!("{id}.{extension}"));
        if is_populated(&local) {
            return Ok(local);
        }

        let cache = cache_dir()
            .join("sprites")
            .join(game)
            .join(&variant)
            .join(format!("{id}.{extension}"));
        if is_populated(&cache) {
            return Ok(cache);
        }

        if let Some(bytes) = bundled::sprite(game, &variant, &id.to_string()) {
            atomic_write(&cache, bytes)?;
            return Ok(cache);
        }

        let primary_url = self.url(id)?;
        let bytes = match download(&primary_url) {
            Ok(bytes) => bytes,
            Err(primary_error) if extension == "png" && variant != "default" => {
                let fallback_url = format!("{SPRITE_BASE_URL}/{id}.png");
                download(&fallback_url).with_context(|| {
                    format!(
                        "fetching {variant} sprite failed ({primary_error}); default fallback also failed"
                    )
                })?
            }
            Err(error) => return Err(error),
        };
        image::load_from_memory(&bytes).context("downloaded sprite was not a valid image")?;
        atomic_write(&cache, &bytes)?;
        Ok(cache)
    }

    pub fn variant(&self) -> String {
        if self.config.artwork {
            "official-artwork".to_string()
        } else if known_game(self.config.variant.trim()).is_some() {
            preferred_front(self.config.variant.trim()).to_string()
        } else if self.config.variant.trim().is_empty() {
            preferred_front(self.game()).to_string()
        } else {
            self.config.variant.trim().to_string()
        }
    }

    pub fn game(&self) -> &str {
        &self.game
    }

    pub fn has_bundled_sprite(&self, id: u16) -> bool {
        bundled::sprite(&self.game, &self.variant(), &id.to_string()).is_some()
    }

    pub fn label(&self) -> String {
        if self.config.artwork {
            "official-artwork".to_string()
        } else {
            format!("{}/{}", self.game(), self.variant())
        }
    }

    fn url(&self, id: u16) -> Result<String> {
        if self.config.artwork {
            return Ok(format!("{SPRITE_BASE_URL}/other/official-artwork/{id}.png"));
        }

        let game = self.game();
        let variant = self.variant();
        let generation = generation_for(game).expect("validated game mapping");
        let source = source_for_variant(game, &variant)
            .with_context(|| format!("no PokeAPI source mapping for sprite variant {variant:?}"))?;
        let suffix = if source.is_empty() {
            String::new()
        } else {
            format!("/{source}")
        };
        Ok(format!(
            "{SPRITE_BASE_URL}/versions/{generation}/{game}{suffix}/{id}.{}",
            extension_for(&variant)
        ))
    }
}

fn choose_bundled_game(
    variant: &str,
    species: Option<u16>,
    requested: Option<&[String]>,
) -> Result<String> {
    if let Some(requested) = requested {
        let missing = requested
            .iter()
            .map(|game| game.trim())
            .filter(|game| !bundled::GAMES.contains(game))
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing.is_empty(),
            "the compiled {} bundle does not contain: {}",
            bundled::PROFILE,
            missing.join(", ")
        );
    }
    let override_game = std::env::var("POKEFETCH_GAME_OVERRIDE").ok();
    let candidates = bundled::GAMES
        .iter()
        .copied()
        .filter(|game| {
            requested.is_none_or(|games| games.iter().any(|wanted| wanted.trim() == *game))
        })
        .filter(|game| {
            species.map_or_else(
                || has_any_bundled_sprite(game, variant),
                |id| bundled::sprite(game, variant, &id.to_string()).is_some(),
            )
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !candidates.is_empty(),
        "the compiled {} bundle has no {variant} sprites matching this selection",
        bundled::PROFILE
    );
    if let Some(game) = override_game.filter(|game| candidates.contains(&game.as_str())) {
        return Ok(game);
    }
    let index = rand::rng().random_range(0..candidates.len());
    Ok(candidates[index].to_string())
}

fn has_any_bundled_sprite(game: &str, variant: &str) -> bool {
    (1..=1025).any(|id| bundled::sprite(game, variant, &id.to_string()).is_some())
}

fn known_game(value: &str) -> Option<&str> {
    generation_for(value).map(|_| value)
}

fn preferred_front(game: &str) -> &'static str {
    let _ = game;
    "front"
}

fn extension_for(variant: &str) -> &'static str {
    if variant.contains("animated") {
        "gif"
    } else {
        "png"
    }
}

fn source_for_variant(game: &str, variant: &str) -> Option<&'static str> {
    match (game, variant) {
        ("red-blue" | "yellow" | "gold" | "silver" | "crystal", "front") => Some("transparent"),
        (_, "front") => Some(""),
        (_, "front-animated") => Some("animated"),
        _ => None,
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
    use super::{generation_for, preferred_front, source_for_variant, SpriteStore};
    #[cfg(any(feature = "bundle-gen1", feature = "bundle-assets"))]
    use crate::config::GameSelection;
    use crate::config::SpriteConfig;
    use std::path::Path;

    #[test]
    fn maps_the_checked_in_sprite_variant() {
        assert_eq!(generation_for("red-blue"), Some("generation-i"));
        assert_eq!(generation_for("crystal"), Some("generation-ii"));
        assert_eq!(generation_for("made-up"), None);
    }

    #[test]
    fn maps_asset_variants_and_legacy_game_configuration() {
        assert_eq!(source_for_variant("crystal", "front"), Some("transparent"));
        assert_eq!(source_for_variant("emerald", "front"), Some(""));
        assert_eq!(preferred_front("emerald"), "front");
        let config = SpriteConfig {
            variant: "crystal".to_string(),
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        assert_eq!(store.game(), "crystal");
        assert_eq!(store.variant(), "front");
    }

    #[cfg(feature = "bundle-gen1")]
    #[test]
    fn random_game_uses_a_game_present_in_the_bundle() {
        let config = SpriteConfig {
            game: "random".into(),
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        assert_eq!(store.game(), "red-blue");
        assert!(store.has_bundled_sprite(25));
    }

    #[cfg(any(feature = "bundle-gen1", feature = "bundle-assets"))]
    #[test]
    fn curated_pool_uses_only_requested_bundled_games() {
        let requested = super::bundled::GAMES
            .iter()
            .take(2)
            .map(|game| (*game).to_string())
            .collect::<Vec<_>>();
        let config = SpriteConfig {
            game: GameSelection::Many(requested.clone()),
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        assert!(requested.iter().any(|game| game == store.game()));
    }

    #[cfg(feature = "bundle-gen1")]
    #[test]
    fn listed_games_must_be_present_in_the_bundle() {
        let config = SpriteConfig {
            game: GameSelection::Many(vec!["crystal".to_string()]),
            ..SpriteConfig::default()
        };
        let error = SpriteStore::new(&config, Path::new("."), None)
            .err()
            .unwrap();
        assert!(error.to_string().contains("does not contain: crystal"));
    }
}
