//! Finding a sprite's pixels, wherever they happen to live.
//!
//! There are four possible sources, tried in this order:
//!
//! ```text
//!   1. local override   ~/.config/pokefetch/sprites/<game>/<variant>/<id>.png
//!   2. cache            ~/.cache/pokefetch/sprites/<game>/<variant>/<id>.png
//!   3. compiled bundle  baked into the executable at build time
//!   4. `PokeAPI`          downloaded once, then written into the cache
//! ```
//!
//! Only step 4 touches the network, and it is the only step that can be slow.
//! A greeting on a bundled build never reaches it, which is the whole point:
//! shell startup must not depend on a working internet connection.
//!
//! Note that step 3 *writes into* the cache rather than returning bytes
//! directly, so that every later step has a real file path to hand to the
//! image decoder.
//!
//! # Rust concepts on display
//!
//! - **Lifetime parameters**: `SpriteStore<'a>` borrows its config instead of
//!   owning a copy. The `'a` is a compile-time promise that the store cannot
//!   outlive what it points at.
//! - **`include!` of generated code**: the [`bundled`] module is written by
//!   `build.rs` into `OUT_DIR` and pulled in here. This is the standard way to
//!   generate Rust at build time.
//! - **Atomic file writes**: [`atomic_write`] writes to a temporary file and
//!   renames it, so a reader never sees a half-written sprite. Two shells
//!   starting at once is a real scenario, not a hypothetical one.
//! - **Matching on an error to retry**: [`SpriteStore::resolve`] falls back to
//!   a default sprite URL only for a specific failure shape.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::Rng;

use crate::config::{cache_dir, SpriteConfig};
use crate::palette::{Color, SIZE as PALETTE_SIZE};
use crate::pokemon::MAX_DEX_ID;

/// Sprite data compiled into this executable.
///
/// `build.rs` generates this file from the asset manifest and the selected
/// `POKEFETCH_BUNDLE` profile. Without a bundle feature it is generated as a
/// set of stubs that always return [`None`], so the rest of the crate needs no
/// conditional compilation at all.
mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundled.rs"));
}

/// Pinned upstream revision. Every downloaded sprite comes from this commit,
/// so a cache built months apart still holds identical bytes.
const SPRITE_BASE_URL: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/c10459b9b0129eaca5c5d9b1cac65336debb1d08/sprites/pokemon";

/// The only variant Pokefetch renders today.
const FRONT: &str = "front";

/// Returns the palette baked in beside a bundled sprite, if there is one.
///
/// Bundled palettes are computed once at build time, so a greeting never runs
/// the color extractor. Returns [`None`] for sprites that were downloaded or
/// locally overridden — those get extracted at runtime.
pub fn bundled_palette(id: u16, game: &str, variant: &str) -> Option<[Color; PALETTE_SIZE]> {
    bundled::palette(game, variant, &id.to_string())
        .map(|colors| colors.map(|(red, green, blue)| Color::rgb(red, green, blue)))
}

/// Names the bundle profile compiled into this executable, or `none`.
pub fn bundle_profile() -> &'static str {
    bundled::PROFILE
}

/// Knows which game and variant to draw from, and how to get the bytes.
///
/// Built once per run. Construction is where a `random` or pooled game choice
/// is actually resolved, so the same store answers consistently afterwards.
pub struct SpriteStore<'a> {
    config: &'a SpriteConfig,
    config_dir: &'a Path,
    game: String,
}

impl<'a> SpriteStore<'a> {
    /// Builds a store, resolving any random or pooled game choice now.
    ///
    /// `species` narrows that choice to games that actually have artwork for
    /// one Pokemon. Pass [`None`] when the species is not yet decided.
    ///
    /// # Errors
    ///
    /// Returns an error if a requested game is absent from the compiled
    /// bundle, or if no bundled game has a matching sprite.
    pub fn new(
        config: &'a SpriteConfig,
        config_dir: &'a Path,
        species: Option<u16>,
    ) -> Result<Self> {
        // Older configs wrote the game name into `variant`; honor that.
        let legacy_game = known_game(config.variant.trim());
        let configured_game = config.game.fixed().and_then(known_game);
        let variant = if config.artwork {
            "official-artwork"
        } else if legacy_game.is_some() || config.variant.trim().is_empty() {
            FRONT
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

    /// Returns a path to the sprite's bytes, fetching them if necessary.
    ///
    /// Walks the four sources described in the [module docs](self). A bundled
    /// sprite is written into the cache so the caller always gets a file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the sprite is absent everywhere and the download
    /// fails, or if the downloaded bytes are not a decodable image.
    pub fn resolve(&self, id: u16) -> Result<PathBuf> {
        let game = self.game();
        let variant = self.variant();
        let extension = extension_for(&variant);
        let relative = PathBuf::from(game)
            .join(&variant)
            .join(format!("{id}.{extension}"));

        let local = self.config_dir.join("sprites").join(&relative);
        if is_populated(&local) {
            return Ok(local);
        }

        let cache = cache_dir().join("sprites").join(&relative);
        if is_populated(&cache) {
            return Ok(cache);
        }

        if let Some(bytes) = bundled::sprite(game, &variant, &id.to_string()) {
            atomic_write(&cache, bytes)?;
            return Ok(cache);
        }

        let bytes = self.download_sprite(id, &variant, extension)?;
        // Validate before caching, so a captive-portal HTML page never gets
        // stored where it would fail to decode on every future run.
        image::load_from_memory(&bytes).context("downloaded sprite was not a valid image")?;
        atomic_write(&cache, &bytes)?;
        Ok(cache)
    }

    /// Downloads a sprite, retrying against the default artwork on failure.
    ///
    /// Some species have no artwork in some games. Matching on the error shape
    /// — rather than any error — keeps the retry narrow.
    fn download_sprite(&self, id: u16, variant: &str, extension: &str) -> Result<Vec<u8>> {
        let primary_url = self.url(id)?;
        match download(&primary_url) {
            Ok(bytes) => Ok(bytes),
            Err(primary_error) if extension == "png" && variant != "default" => {
                let fallback_url = format!("{SPRITE_BASE_URL}/{id}.png");
                download(&fallback_url).with_context(|| {
                    format!(
                        "fetching {variant} sprite failed ({primary_error}); default fallback also failed"
                    )
                })
            }
            Err(error) => Err(error),
        }
    }

    /// The resolved sprite variant, e.g. `front` or `official-artwork`.
    ///
    /// Every currently bundled game uses `front`, so a legacy config naming a
    /// game in the `variant` field resolves to it.
    pub fn variant(&self) -> String {
        let configured = self.config.variant.trim();
        if self.config.artwork {
            "official-artwork".to_string()
        } else if configured.is_empty() || known_game(configured).is_some() {
            FRONT.to_string()
        } else {
            configured.to_string()
        }
    }

    /// The resolved game, e.g. `crystal`.
    pub fn game(&self) -> &str {
        &self.game
    }

    /// Reports whether this game and variant have `id` compiled in.
    pub fn has_bundled_sprite(&self, id: u16) -> bool {
        bundled::sprite(&self.game, &self.variant(), &id.to_string()).is_some()
    }

    /// A human-readable source label for the greeting, e.g. `crystal/front`.
    pub fn label(&self) -> String {
        if self.config.artwork {
            "official-artwork".to_string()
        } else {
            format!("{}/{}", self.game(), self.variant())
        }
    }

    /// Builds the upstream URL for one sprite.
    ///
    /// # Errors
    ///
    /// Returns an error if the variant has no known upstream location.
    fn url(&self, id: u16) -> Result<String> {
        if self.config.artwork {
            return Ok(format!("{SPRITE_BASE_URL}/other/official-artwork/{id}.png"));
        }

        let game = self.game();
        let variant = self.variant();
        let generation = generation_for(game).context("sprite game is not in the catalog")?;
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

/// Picks a game from the compiled bundle that satisfies the request.
///
/// `POKEFETCH_GAME_OVERRIDE` pins the result, which is how background icon
/// generation draws the same game the greeting just chose.
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

/// Reports whether a game has any sprite at all in this build.
///
/// A linear scan over the Pokedex. `any` short-circuits on the first hit, so
/// in practice this stops within a handful of lookups.
fn has_any_bundled_sprite(game: &str, variant: &str) -> bool {
    (1..=MAX_DEX_ID).any(|id| bundled::sprite(game, variant, &id.to_string()).is_some())
}

/// Returns the input unchanged if it names a game Pokefetch can locate.
fn known_game(value: &str) -> Option<&str> {
    generation_for(value).map(|_| value)
}

/// File extension for a variant. Only animated variants are GIFs.
fn extension_for(variant: &str) -> &'static str {
    if variant.contains("animated") {
        "gif"
    } else {
        "png"
    }
}

/// Maps a game and variant to its `PokeAPI` subdirectory.
///
/// Generations I and II keep transparent renderings in a `transparent`
/// subdirectory; Generation III sprites already carry alpha and sit at the
/// top level, which is what the empty string means here.
fn source_for_variant(game: &str, variant: &str) -> Option<&'static str> {
    match (game, variant) {
        ("red-blue" | "yellow" | "gold" | "silver" | "crystal", FRONT) => Some("transparent"),
        (_, FRONT) => Some(""),
        (_, "front-animated") => Some("animated"),
        _ => None,
    }
}

/// Maps a game to the `PokeAPI` generation directory that holds its sprites.
///
/// Generations IV and later are listed because the URL scheme is known, even
/// though no bundle imports them yet.
fn generation_for(game: &str) -> Option<&'static str> {
    match game {
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

/// Fetches a URL into memory.
fn download(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header(
            "User-Agent",
            concat!("pokefetch/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .with_context(|| format!("downloading {url}"))?;
    response
        .body_mut()
        .read_to_vec()
        .with_context(|| format!("reading {url}"))
}

/// Writes bytes to `path` so that readers see either nothing or the whole file.
///
/// Write to a uniquely named temporary in the *same directory*, then rename.
/// Rename is atomic within a filesystem, which is why the temporary cannot go
/// in `/tmp`. The process id and a random nonce keep two concurrent shells
/// from colliding.
///
/// A failed rename where the destination now exists is treated as success:
/// another process won the race and wrote the same bytes.
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
        Err(_) if is_populated(path) => {
            let _ = std::fs::remove_file(&temporary);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error).with_context(|| format!("installing {}", path.display()))
        }
    }
}

/// Reports whether a path is a file with content.
///
/// Length matters, not just existence: an interrupted write can leave a
/// zero-byte file behind, and treating that as a cache hit would be wrong.
fn is_populated(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| metadata.len() > 0)
}

#[cfg(test)]
mod tests {
    use super::{extension_for, generation_for, source_for_variant, SpriteStore};
    use crate::config::{GameSelection, SpriteConfig};
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
        assert_eq!(source_for_variant("emerald", "back"), None);
        let config = SpriteConfig {
            variant: "crystal".to_string(),
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        assert_eq!(store.game(), "crystal");
        assert_eq!(store.variant(), "front");
    }

    #[test]
    fn only_animated_variants_are_gifs() {
        assert_eq!(extension_for("front"), "png");
        assert_eq!(extension_for("official-artwork"), "png");
        assert_eq!(extension_for("front-animated"), "gif");
    }

    #[test]
    fn artwork_overrides_the_configured_variant() {
        let config = SpriteConfig {
            artwork: true,
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        assert_eq!(store.variant(), "official-artwork");
        assert_eq!(store.label(), "official-artwork");
    }

    #[cfg(feature = "bundle-assets")]
    #[test]
    fn random_game_uses_a_game_present_in_the_bundle() {
        let config = SpriteConfig {
            game: "random".into(),
            ..SpriteConfig::default()
        };
        let store = SpriteStore::new(&config, Path::new("."), None).unwrap();
        // Which game is chosen depends on POKEFETCH_BUNDLE, so assert the
        // property that must hold for every profile rather than a fixed name.
        assert!(super::bundled::GAMES.contains(&store.game()));
    }

    #[cfg(feature = "bundle-assets")]
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

    #[test]
    fn listed_games_must_be_present_in_the_bundle() {
        // Deliberately a name no profile can contain, so this holds for every
        // bundle -- including the stub bundle of a default build.
        let config = SpriteConfig {
            game: GameSelection::Many(vec!["not-a-bundled-game".to_string()]),
            ..SpriteConfig::default()
        };
        let error = SpriteStore::new(&config, Path::new("."), None)
            .err()
            .unwrap();
        assert!(error
            .to_string()
            .contains("does not contain: not-a-bundled-game"));
    }
}
