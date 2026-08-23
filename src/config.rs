use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sprites: SpriteConfig,
    pub display: DisplayConfig,
    pub icon: IconConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SpriteConfig {
    pub game: String,
    pub variant: String,
    pub artwork: bool,
    pub range_start: u16,
    pub range_end: u16,
    pub pokemon: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub columns: u16,
    pub rows: u16,
    pub gap: u16,
    pub canvas_pixels: u32,
    pub background: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct IconConfig {
    pub enabled: bool,
}

impl Default for SpriteConfig {
    fn default() -> Self {
        Self {
            game: "red-blue".to_string(),
            variant: "front".to_string(),
            artwork: false,
            range_start: 1,
            range_end: 151,
            pokemon: Vec::new(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            columns: 18,
            rows: 9,
            gap: 2,
            canvas_pixels: 288,
            background: "#222436".to_string(),
        }
    }
}

impl Default for IconConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Config {
    pub fn load() -> Result<(Self, PathBuf)> {
        let dir = config_dir();
        let path = dir.join("config.toml");
        let config = if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?
        } else {
            Self::default()
        };
        Ok((config, dir))
    }

    pub fn validate(&self) -> Result<()> {
        let game = self.sprites.game.trim();
        let variant = self.sprites.variant.trim();
        let legacy_game = is_supported_game(variant);
        anyhow::ensure!(
            is_supported_game(game),
            "sprites.game must name a cataloged game"
        );
        anyhow::ensure!(
            self.sprites.artwork || legacy_game || matches!(variant, "front" | "front-animated"),
            "sprites.variant must be front or front-animated"
        );
        anyhow::ensure!(
            variant != "front-animated" || game == "crystal",
            "front-animated is available only for crystal"
        );
        anyhow::ensure!(
            self.sprites.range_start > 0 && self.sprites.range_start <= self.sprites.range_end,
            "sprites.range_start must be positive and no greater than range_end"
        );
        anyhow::ensure!(
            self.sprites.range_end <= 1025,
            "sprites.range_end must not exceed 1025"
        );
        anyhow::ensure!(
            self.sprites.pokemon.iter().all(|id| *id > 0 && *id <= 1025),
            "sprites.pokemon entries must be between 1 and 1025"
        );
        anyhow::ensure!(
            self.display.columns > 0 && self.display.rows >= 6,
            "display needs at least one column and six rows"
        );
        anyhow::ensure!(
            self.display.canvas_pixels >= 32,
            "display.canvas_pixels must be at least 32"
        );
        Ok(())
    }
}

fn is_supported_game(value: &str) -> bool {
    matches!(
        value,
        "red-blue"
            | "yellow"
            | "gold"
            | "silver"
            | "crystal"
            | "ruby-sapphire"
            | "emerald"
            | "firered-leafgreen"
    )
}

pub fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path).join("pokefetch")
    } else {
        home_dir().join(".config/pokefetch")
    }
}

pub fn cache_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(path).join("pokefetch")
    } else {
        home_dir().join(".cache/pokefetch")
    }
}

pub fn state_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(path).join("pokefetch")
    } else {
        home_dir().join(".local/state/pokefetch")
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn defaults_match_the_checked_in_profile() {
        let config = Config::default();
        assert_eq!(config.sprites.game, "red-blue");
        assert_eq!(config.sprites.variant, "front");
        assert_eq!(config.sprites.range_start, 1);
        assert_eq!(config.sprites.range_end, 151);
        assert_eq!(config.display.rows, 9);
        assert!(config.icon.enabled);
    }

    #[test]
    fn rejects_an_inverted_random_range() {
        let mut config = Config::default();
        config.sprites.range_start = 151;
        config.sprites.range_end = 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_game_and_active_variant_names() {
        let mut config = Config::default();
        config.sprites.game = "crystl".to_string();
        assert!(config.validate().is_err());

        config.sprites.game = "crystal".to_string();
        config.sprites.variant = "front-animated".to_string();
        assert!(config.validate().is_ok());
    }
}
