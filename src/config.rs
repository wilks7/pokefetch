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
    pub game: GameSelection,
    pub variant: String,
    pub artwork: bool,
    pub range_start: u16,
    pub range_end: u16,
    pub pokemon: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum GameSelection {
    One(String),
    Many(Vec<String>),
}

impl GameSelection {
    pub fn fixed(&self) -> Option<&str> {
        match self {
            Self::One(game) if game.trim() != "random" => Some(game.trim()),
            _ => None,
        }
    }

    pub fn is_pool(&self) -> bool {
        !matches!(self, Self::One(game) if game.trim() != "random")
    }

    pub fn pool(&self) -> Option<&[String]> {
        match self {
            Self::Many(games) => Some(games),
            Self::One(_) => None,
        }
    }
}

impl From<&str> for GameSelection {
    fn from(game: &str) -> Self {
        Self::One(game.to_string())
    }
}

impl From<String> for GameSelection {
    fn from(game: String) -> Self {
        Self::One(game)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub size: u16,
    pub alignment: Alignment,
    pub gap: u16,
    pub background: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Top,
    #[default]
    Center,
}

impl DisplayConfig {
    pub fn columns(&self) -> u32 {
        u32::from(self.size) * 2
    }

    pub fn canvas_pixels(&self) -> u32 {
        (u32::from(self.size) * 32).min(2048)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct IconConfig {
    pub enabled: bool,
}

impl Default for SpriteConfig {
    fn default() -> Self {
        Self {
            game: "red-blue".into(),
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
            size: 8,
            alignment: Alignment::Center,
            gap: 2,
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
        let game = self.sprites.game.fixed();
        let variant = self.sprites.variant.trim();
        let legacy_game = is_supported_game(variant);
        match &self.sprites.game {
            GameSelection::One(game) => anyhow::ensure!(
                game.trim() == "random" || is_supported_game(game.trim()),
                "sprites.game must name a cataloged game, random, or a list of games"
            ),
            GameSelection::Many(games) => {
                anyhow::ensure!(!games.is_empty(), "sprites.game list must not be empty");
                anyhow::ensure!(
                    games.iter().all(|game| is_supported_game(game.trim())),
                    "every sprites.game list entry must name a cataloged game"
                );
                let unique = games
                    .iter()
                    .map(|game| game.trim())
                    .collect::<std::collections::BTreeSet<_>>();
                anyhow::ensure!(
                    unique.len() == games.len(),
                    "sprites.game list must not contain duplicates"
                );
            }
        }
        anyhow::ensure!(
            self.sprites.artwork || legacy_game || matches!(variant, "front" | "front-animated"),
            "sprites.variant must be front or front-animated"
        );
        anyhow::ensure!(
            variant != "front-animated" || game == Some("crystal"),
            "front-animated is available only for crystal"
        );
        anyhow::ensure!(
            !self.sprites.game.is_pool() || (!self.sprites.artwork && variant == "front"),
            "random or listed sprites.game selections require bundled front sprites"
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
            self.display.size > 0,
            "display.size must be at least one row"
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
    use super::{Alignment, Config, GameSelection};

    #[test]
    fn defaults_match_the_checked_in_profile() {
        let config = Config::default();
        assert_eq!(config.sprites.game.fixed(), Some("red-blue"));
        assert_eq!(config.sprites.variant, "front");
        assert_eq!(config.sprites.range_start, 1);
        assert_eq!(config.sprites.range_end, 151);
        assert_eq!(config.display.size, 8);
        assert_eq!(config.display.alignment, Alignment::Center);
        assert!(config.icon.enabled);
    }

    #[test]
    fn an_empty_toml_document_uses_runnable_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.sprites.game.fixed(), Some("red-blue"));
        assert_eq!(config.sprites.range_end, 151);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn accepts_large_row_sizes_with_a_bounded_render_canvas() {
        let mut config = Config::default();
        config.display.size = 64;
        assert!(config.validate().is_ok());
        assert_eq!(config.display.columns(), 128);
        assert_eq!(config.display.canvas_pixels(), 2048);

        config.display.size = 128;
        assert!(config.validate().is_ok());
        assert_eq!(config.display.canvas_pixels(), 2048);

        config.display.size = 0;
        assert!(config.validate().is_err());
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
        config.sprites.game = "crystl".into();
        assert!(config.validate().is_err());

        config.sprites.game = "crystal".into();
        config.sprites.variant = "front-animated".to_string();
        assert!(config.validate().is_ok());

        config.sprites.game = "random".into();
        config.sprites.variant = "front".to_string();
        assert!(config.validate().is_ok());

        config.sprites.game =
            GameSelection::Many(vec!["crystal".to_string(), "emerald".to_string()]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn parses_a_curated_game_pool() {
        let config: Config = toml::from_str(
            r#"
            [sprites]
            game = ["gold", "silver", "crystal"]
            "#,
        )
        .unwrap();
        assert_eq!(config.sprites.game.pool().unwrap().len(), 3);
        assert!(config.validate().is_ok());
    }
}
