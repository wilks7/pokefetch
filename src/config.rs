//! Settings: where they come from, what they default to, and what is legal.
//!
//! The config file is optional. With no file at all Pokefetch runs with the
//! defaults defined here, which is a deliberate property — a tool that greets
//! you at shell startup should not require setup before it works.
//!
//! ```toml
//! # ~/.config/pokefetch/config.toml
//! [sprites]
//! game = "crystal"
//! range_end = 251
//!
//! [display]
//! size = 8
//! alignment = "center"
//! ```
//!
//! # Rust concepts on display
//!
//! - **`#[serde(default)]`**: applied to a struct, every missing field falls
//!   back to that type's [`Default`]. This is what makes an empty file, a
//!   partial file, and no file behave identically.
//! - **`#[serde(untagged)]`**: [`GameSelection`] accepts either a string or a
//!   list from TOML. Serde tries each variant in order and keeps the first
//!   that parses.
//! - **Manual `Default` impls**: derived `Default` gives 0 and `""`, which
//!   would be an invalid config. The impls below spell out a *runnable* one.
//! - **Parse, don't validate — nearly**: [`Config::validate`] is a separate
//!   step rather than being enforced by the types. That is a pragmatic
//!   trade-off; the type-driven alternative is discussed in `docs/tour/`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::pokemon::MAX_DEX_ID;

/// Largest sprite height, in terminal rows.
///
/// Above this the derived pixel canvas grows past any sensible terminal.
pub const MAX_SIZE_ROWS: u16 = 32;

/// Games with artwork Pokefetch knows how to locate.
const SUPPORTED_GAMES: [&str; 8] = [
    "red-blue",
    "yellow",
    "gold",
    "silver",
    "crystal",
    "ruby-sapphire",
    "emerald",
    "firered-leafgreen",
];

/// The whole configuration, matching the shape of `config.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Which artwork to draw.
    pub sprites: SpriteConfig,
    /// How to lay it out.
    pub display: DisplayConfig,
    /// Ghostty icon behaviour.
    pub icon: IconConfig,
}

/// The `[sprites]` table: which species, from which game, in which variant.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SpriteConfig {
    /// A game name, the word `random`, or a list of games.
    pub game: GameSelection,
    /// Sprite variant, normally `front`.
    pub variant: String,
    /// Use high-resolution official artwork instead of game sprites.
    pub artwork: bool,
    /// Lowest Pokedex number eligible for random selection.
    pub range_start: u16,
    /// Highest Pokedex number eligible for random selection.
    pub range_end: u16,
    /// An explicit species list, which takes priority over the range.
    pub pokemon: Vec<u16>,
}

/// How `sprites.game` was written in the config file.
///
/// TOML has no sum types, so the same key accepts three different shapes:
///
/// ```toml
/// game = "crystal"                      # One, fixed
/// game = "random"                       # One, but means "any bundled game"
/// game = ["gold", "silver", "crystal"]  # Many, a curated pool
/// ```
///
/// `#[serde(untagged)]` is what lets one field accept both a string and a
/// list: serde attempts each variant and keeps the first that deserializes.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum GameSelection {
    /// A single name, which may be the special value `random`.
    One(String),
    /// An explicit pool to choose from.
    Many(Vec<String>),
}

impl GameSelection {
    /// Returns the game name when exactly one specific game was requested.
    ///
    /// Returns [`None`] for `random` and for lists, since neither names one
    /// game until selection actually runs.
    ///
    /// ```
    /// # use pokefetch::config::GameSelection;
    /// assert_eq!(GameSelection::from("crystal").fixed(), Some("crystal"));
    /// assert_eq!(GameSelection::from("random").fixed(), None);
    /// ```
    pub fn fixed(&self) -> Option<&str> {
        match self {
            Self::One(game) if game.trim() != "random" => Some(game.trim()),
            _ => None,
        }
    }

    /// Reports whether selection has to choose among several games.
    ///
    /// The exact inverse of [`fixed`](Self::fixed) being `Some`.
    pub fn is_pool(&self) -> bool {
        self.fixed().is_none()
    }

    /// Returns the explicit pool, if one was written as a list.
    ///
    /// `random` is a pool too, but an implicit one — it means "whatever the
    /// compiled bundle contains" — so it returns [`None`] here.
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

/// The `[display]` table: size, alignment, spacing, and background.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Sprite height in terminal rows.
    pub size: u16,
    /// How to align the image against the text.
    pub alignment: Alignment,
    /// Blank columns between the image and the text.
    pub gap: u16,
    /// Terminal background as `#RRGGBB`, used for contrast correction.
    pub background: String,
}

/// Vertical alignment of the image against the text.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    /// Both start on the first row.
    Top,
    /// Whichever is shorter is centered against the other.
    #[default]
    Center,
}

impl DisplayConfig {
    /// Width of the image placement, in terminal columns.
    ///
    /// Terminal cells are about twice as tall as they are wide, so a square
    /// sprite needs twice as many columns as rows to look square.
    ///
    /// ```
    /// # use pokefetch::config::DisplayConfig;
    /// let display = DisplayConfig::default();   // size = 8
    /// assert_eq!(display.columns(), 16);
    /// assert_eq!(display.canvas_pixels(), 256);
    /// ```
    pub fn columns(&self) -> u32 {
        u32::from(self.size) * 2
    }

    /// Side length of the square pixel canvas to render into.
    ///
    /// Deriving this from the row count instead of hardcoding pixels is what
    /// keeps a sprite crisp when the terminal font size changes.
    pub fn canvas_pixels(&self) -> u32 {
        u32::from(self.size) * 32
    }
}

/// The `[icon]` table.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct IconConfig {
    /// Prepare Ghostty's next icon during a greeting.
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
    /// Loads the config file, falling back to defaults when it is absent.
    ///
    /// Returns the config alongside its directory, because local sprite
    /// overrides are resolved relative to that directory.
    ///
    /// A *missing* file is normal and silently uses defaults; a file that
    /// exists but does not parse is an error, because that is a typo the user
    /// wants to hear about rather than have ignored.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
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

    /// Checks that the merged configuration describes something drawable.
    ///
    /// Split into three helpers by concern; each returns on its first problem
    /// so the user sees one actionable message rather than a wall of them.
    ///
    /// # Errors
    ///
    /// Returns an error naming the first setting that is out of range or
    /// inconsistent with another.
    pub fn validate(&self) -> Result<()> {
        self.validate_games()?;
        self.validate_variant()?;
        self.validate_ranges()
    }

    /// Checks `sprites.game` against the catalog of known games.
    fn validate_games(&self) -> Result<()> {
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
                // A BTreeSet drops duplicates, so a shrinking length means the
                // list repeated a game.
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
        Ok(())
    }

    /// Checks that the variant exists and is compatible with the game choice.
    fn validate_variant(&self) -> Result<()> {
        let variant = self.sprites.variant.trim();
        // Older configs put the game name in `variant`. Accept it so that
        // upgrading Pokefetch does not break an existing config file.
        let legacy_game = is_supported_game(variant);
        anyhow::ensure!(
            self.sprites.artwork || legacy_game || matches!(variant, "front" | "front-animated"),
            "sprites.variant must be front or front-animated"
        );
        anyhow::ensure!(
            variant != "front-animated" || self.sprites.game.fixed() == Some("crystal"),
            "front-animated is available only for crystal"
        );
        anyhow::ensure!(
            !self.sprites.game.is_pool() || (!self.sprites.artwork && variant == "front"),
            "random or listed sprites.game selections require bundled front sprites"
        );
        Ok(())
    }

    /// Checks the numeric ranges: species ids and display size.
    fn validate_ranges(&self) -> Result<()> {
        anyhow::ensure!(
            self.sprites.range_start > 0 && self.sprites.range_start <= self.sprites.range_end,
            "sprites.range_start must be positive and no greater than range_end"
        );
        anyhow::ensure!(
            self.sprites.range_end <= MAX_DEX_ID,
            "sprites.range_end must not exceed {MAX_DEX_ID}"
        );
        anyhow::ensure!(
            self.sprites
                .pokemon
                .iter()
                .all(|id| *id > 0 && *id <= MAX_DEX_ID),
            "sprites.pokemon entries must be between 1 and {MAX_DEX_ID}"
        );
        anyhow::ensure!(
            (1..=MAX_SIZE_ROWS).contains(&self.display.size),
            "display.size must be between 1 and {MAX_SIZE_ROWS} rows"
        );
        Ok(())
    }
}

/// Reports whether a name appears in [`SUPPORTED_GAMES`].
fn is_supported_game(value: &str) -> bool {
    SUPPORTED_GAMES.contains(&value)
}

/// Directory holding `config.toml`, honoring `XDG_CONFIG_HOME`.
pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config/pokefetch")
}

/// Directory for downloaded sprites and the system snapshot.
pub fn cache_dir() -> PathBuf {
    xdg_dir("XDG_CACHE_HOME", ".cache/pokefetch")
}

/// Directory for generated state, such as the Ghostty icon.
pub fn state_dir() -> PathBuf {
    xdg_dir("XDG_STATE_HOME", ".local/state/pokefetch")
}

/// Resolves one XDG base directory, or a path under `$HOME`.
///
/// `var_os` rather than `var` because paths are not required to be valid
/// UTF-8 on every platform, and rejecting one for that would be wrong.
fn xdg_dir(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable).map_or_else(
        || home_dir().join(fallback),
        |path| PathBuf::from(path).join("pokefetch"),
    )
}

/// The user's home directory, falling back to the working directory.
fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| Path::new(".").to_path_buf(), PathBuf::from)
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
    fn a_partial_table_keeps_the_other_defaults() {
        let config: Config = toml::from_str("[display]\nsize = 4\n").unwrap();
        assert_eq!(config.display.size, 4);
        assert_eq!(config.display.gap, 2, "unset keys keep their default");
        assert_eq!(config.sprites.game.fixed(), Some("red-blue"));
    }

    #[test]
    fn accepts_row_sizes_through_thirty_two() {
        let mut config = Config::default();
        config.display.size = 32;
        assert!(config.validate().is_ok());
        assert_eq!(config.display.columns(), 64);
        assert_eq!(config.display.canvas_pixels(), 1024);

        config.display.size = 33;
        assert!(config.validate().is_err());

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
    fn rejects_a_duplicated_game_pool() {
        let mut config = Config::default();
        config.sprites.game =
            GameSelection::Many(vec!["crystal".to_string(), "crystal".to_string()]);
        assert!(config.validate().is_err());

        config.sprites.game = GameSelection::Many(Vec::new());
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
    fn front_animated_is_limited_to_crystal() {
        let mut config = Config::default();
        config.sprites.game = "emerald".into();
        config.sprites.variant = "front-animated".to_string();
        assert!(config.validate().is_err());
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
