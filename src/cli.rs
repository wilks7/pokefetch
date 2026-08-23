//! The command-line surface, and how flags layer over the config file.
//!
//! Pokefetch resolves settings in three stages, each overriding the last:
//!
//! ```text
//!   built-in defaults   ->   ~/.config/pokefetch/config.toml   ->   CLI flags
//! ```
//!
//! Flags never rewrite the file. `pokefetch --size 2` changes this one run and
//! nothing else, which is what makes the command safe to experiment with.
//!
//! # Rust concepts on display
//!
//! - **Derive macros**: `#[derive(Parser)]` generates an argument parser from
//!   the struct definition at compile time. The struct *is* the specification.
//! - **Doc comments as behaviour**: clap turns each `///` below into the help
//!   text for that flag. Documenting a field and implementing `--help` are the
//!   same act here, which is why every field has one.
//! - **The `From` trait**: [`AlignmentArg`] converts into
//!   [`Alignment`] via `impl From`, which is how Rust
//!   spells "this type can become that one".

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

use crate::config::{Alignment, Config, GameSelection};

/// Renders a Pokemon sprite and your machine's details in the terminal.
///
/// Every option is global, so `pokefetch --size 4 show pikachu` and
/// `pokefetch show pikachu --size 4` mean the same thing. Flags override the
/// config file for one run and never rewrite it.
// clap needs one field per flag, and `--icon`/`--no-icon` pairs are the
// clearest way to express an override that can also be "unspecified". That
// exceeds clippy's preferred bool count, and the alternative (a nested struct
// per pair) would obscure the generated help more than it would help readers.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Select one game; repeat or use commas for a curated pool.
    #[arg(long, global = true, value_delimiter = ',', action = ArgAction::Append)]
    pub game: Vec<String>,

    /// Sprite variant to render: `front` or `front-animated`.
    #[arg(long, global = true)]
    pub variant: Option<String>,

    /// Use high-resolution official artwork instead of game sprites.
    #[arg(long, global = true, conflicts_with = "no_artwork")]
    pub artwork: bool,

    /// Use game sprites, overriding `sprites.artwork` in the config file.
    #[arg(long, global = true, conflicts_with = "artwork")]
    pub no_artwork: bool,

    /// Lowest Pokedex number eligible for random selection.
    #[arg(long, global = true)]
    pub range_start: Option<u16>,

    /// Highest Pokedex number eligible for random selection.
    #[arg(long, global = true)]
    pub range_end: Option<u16>,

    /// Set the sprite height in terminal rows (1-32).
    #[arg(long, global = true)]
    pub size: Option<u16>,

    /// Align the image and text to the top, or center the shorter one.
    #[arg(long, global = true)]
    pub alignment: Option<AlignmentArg>,

    /// Blank columns between the sprite and the text.
    #[arg(long, global = true)]
    pub gap: Option<u16>,

    /// Terminal background as `#RRGGBB`, used to keep palette colors legible.
    #[arg(long, global = true)]
    pub background: Option<String>,

    /// Prepare Ghostty's next icon, even outside a local Ghostty session.
    #[arg(long, global = true, conflicts_with = "no_icon")]
    pub icon: bool,

    /// Skip icon preparation for this run.
    #[arg(long, global = true, conflicts_with = "icon")]
    pub no_icon: bool,

    /// Subcommand to run; defaults to `greet`.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Vertical alignment, as spelled on the command line.
///
/// This mirrors [`Alignment`] rather than reusing it, so that the CLI's
/// vocabulary and the config file's type can evolve independently. `ValueEnum`
/// generates both the parser and the `--help` value list.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AlignmentArg {
    /// Start the image and the text on the same first row.
    Top,
    /// Vertically center whichever of the two is shorter.
    Center,
}

impl From<AlignmentArg> for Alignment {
    fn from(alignment: AlignmentArg) -> Self {
        match alignment {
            AlignmentArg::Top => Self::Top,
            AlignmentArg::Center => Self::Center,
        }
    }
}

/// What Pokefetch should do this run.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show a random Pokemon greeting and prepare Ghostty's next icon.
    Greet {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
        /// Send Kitty graphics even if the terminal does not advertise support.
        #[arg(long)]
        force_kitty: bool,
    },
    /// Show one Pokemon without changing Ghostty's icon.
    Show {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
        /// Send Kitty graphics even if the terminal does not advertise support.
        #[arg(long)]
        force_kitty: bool,
    },
    /// Print the eight terminal colors extracted from a sprite.
    Palette {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
    },
    /// Generate a macOS ICNS file from a sprite.
    Icon {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
        /// Where to write the icon; defaults to the state directory.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Print the resolved local or cached sprite path.
    Sprite {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
    },
    /// Render a cropped nearest-neighbor PNG for inspection.
    Render {
        /// Name, Pokedex number, or `random`.
        pokemon: Option<String>,
        /// Destination PNG path.
        #[arg(long)]
        output: PathBuf,
        /// Square canvas size in pixels.
        #[arg(long, default_value_t = 288)]
        pixels: u32,
    },
    /// Print the sprite bundle profile compiled into this binary.
    Bundle,
}

impl Cli {
    /// Returns the subcommand, defaulting to a plain greeting.
    ///
    /// Running bare `pokefetch` is the common case — it is what a shell
    /// startup file calls — so it gets the default rather than a usage error.
    pub fn command(self) -> Command {
        self.command.unwrap_or(Command::Greet {
            pokemon: None,
            force_kitty: false,
        })
    }
}

/// Applies command-line overrides on top of an already-loaded config.
///
/// `&mut Config` is an *exclusive* borrow: while this function runs, no other
/// code can read or write that config. The compiler enforces it, which is why
/// mutation like this is safe to reason about in Rust.
///
/// Each field follows the same shape — `Option` means "unspecified, leave the
/// configured value alone", and paired booleans resolve through
/// [`overridden_flag`].
pub fn apply_overrides(config: &mut Config, cli: &Cli) {
    if !cli.game.is_empty() {
        config.sprites.game = if cli.game.len() == 1 {
            GameSelection::One(cli.game[0].clone())
        } else {
            GameSelection::Many(cli.game.clone())
        };
    }
    if let Some(variant) = &cli.variant {
        config.sprites.variant.clone_from(variant);
    }
    if let Some(artwork) = overridden_flag(cli.artwork, cli.no_artwork) {
        config.sprites.artwork = artwork;
    }
    if let Some(range_start) = cli.range_start {
        config.sprites.range_start = range_start;
    }
    if let Some(range_end) = cli.range_end {
        config.sprites.range_end = range_end;
    }
    if let Some(size) = cli.size {
        config.display.size = size;
    }
    if let Some(alignment) = cli.alignment {
        config.display.alignment = alignment.into();
    }
    if let Some(gap) = cli.gap {
        config.display.gap = gap;
    }
    if let Some(background) = &cli.background {
        config.display.background.clone_from(background);
    }
    if let Some(enabled) = overridden_flag(cli.icon, cli.no_icon) {
        config.icon.enabled = enabled;
    }
}

/// Collapses an `--x` / `--no-x` flag pair into an optional override.
///
/// Returns [`None`] when neither flag was given, which is the signal to leave
/// the configured value untouched. clap already rejects passing both.
fn overridden_flag(enable: bool, disable: bool) -> Option<bool> {
    match (enable, disable) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_overrides, overridden_flag, Cli};
    use crate::config::{Alignment, Config};
    use clap::Parser;

    #[test]
    fn cli_values_override_config_without_mutating_a_file() {
        let cli = Cli::try_parse_from([
            "pokefetch",
            "--game",
            "gold,crystal",
            "--size",
            "2",
            "--alignment",
            "top",
            "--no-icon",
            "show",
            "celebi",
        ])
        .unwrap();
        let mut config = Config::default();
        apply_overrides(&mut config, &cli);

        assert_eq!(config.sprites.game.pool().unwrap().len(), 2);
        assert_eq!(config.display.size, 2);
        assert_eq!(config.display.alignment, Alignment::Top);
        assert!(!config.icon.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn unspecified_flags_leave_the_configured_value_alone() {
        let cli = Cli::try_parse_from(["pokefetch"]).unwrap();
        let mut config = Config::default();
        let before = config.icon.enabled;
        apply_overrides(&mut config, &cli);
        assert_eq!(config.icon.enabled, before);
        assert_eq!(config.display.size, Config::default().display.size);
    }

    #[test]
    fn global_options_are_accepted_on_either_side_of_a_subcommand() {
        let before = Cli::try_parse_from(["pokefetch", "--size", "4", "show", "pikachu"]).unwrap();
        let after = Cli::try_parse_from(["pokefetch", "show", "pikachu", "--size", "4"]).unwrap();
        assert_eq!(before.size, after.size);
    }

    #[test]
    fn conflicting_flag_pairs_are_rejected_by_the_parser() {
        assert!(Cli::try_parse_from(["pokefetch", "--icon", "--no-icon"]).is_err());
        assert!(Cli::try_parse_from(["pokefetch", "--artwork", "--no-artwork"]).is_err());
    }

    #[test]
    fn flag_pairs_collapse_to_an_optional_override() {
        assert_eq!(overridden_flag(false, false), None);
        assert_eq!(overridden_flag(true, false), Some(true));
        assert_eq!(overridden_flag(false, true), Some(false));
    }
}
