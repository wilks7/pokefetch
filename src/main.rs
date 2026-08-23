mod config;
mod icon;
mod image_ops;
mod palette;
mod pokemon;
mod sprite;
mod terminal;

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};

use crate::config::{state_dir, Alignment, Config, GameSelection};
use crate::pokemon::Pokemon;
use crate::sprite::SpriteStore;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Select one game; repeat or use commas for a curated pool.
    #[arg(long, global = true, value_delimiter = ',', action = ArgAction::Append)]
    game: Vec<String>,
    #[arg(long, global = true)]
    variant: Option<String>,
    #[arg(long, global = true, conflicts_with = "no_artwork")]
    artwork: bool,
    #[arg(long, global = true, conflicts_with = "artwork")]
    no_artwork: bool,
    #[arg(long, global = true)]
    range_start: Option<u16>,
    #[arg(long, global = true)]
    range_end: Option<u16>,
    /// Set the sprite height in terminal rows.
    #[arg(long, global = true)]
    size: Option<u16>,
    #[arg(long, global = true)]
    alignment: Option<AlignmentArg>,
    #[arg(long, global = true)]
    gap: Option<u16>,
    #[arg(long, global = true)]
    background: Option<String>,
    #[arg(long, global = true, conflicts_with = "no_icon")]
    icon: bool,
    #[arg(long, global = true, conflicts_with = "icon")]
    no_icon: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AlignmentArg {
    Top,
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

#[derive(Debug, Subcommand)]
enum Command {
    /// Show a random Pokemon greeting and prepare Ghostty's next icon.
    Greet {
        pokemon: Option<String>,
        #[arg(long)]
        force_kitty: bool,
    },
    /// Show one Pokemon without changing Ghostty's icon.
    Show {
        pokemon: Option<String>,
        #[arg(long)]
        force_kitty: bool,
    },
    /// Print the eight terminal colors extracted from a sprite.
    Palette { pokemon: Option<String> },
    /// Generate a macOS ICNS file from a sprite.
    Icon {
        pokemon: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Print the resolved local or cached sprite path.
    Sprite { pokemon: Option<String> },
    /// Render a cropped nearest-neighbor PNG for inspection.
    Render {
        pokemon: Option<String>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 288)]
        pixels: u32,
    },
    /// Print the sprite bundle profile compiled into this binary.
    Bundle,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("pokefetch: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let (mut config, config_dir) = Config::load()?;
    apply_overrides(&mut config, &cli);
    config.validate()?;

    match cli.command.unwrap_or(Command::Greet {
        pokemon: None,
        force_kitty: false,
    }) {
        Command::Greet {
            pokemon,
            force_kitty,
        } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            show(&config, &store, &pokemon, force_kitty)?;
            if config.icon.enabled && terminal::is_local_ghostty() {
                schedule_icon(pokemon.id, store.game())?;
            }
        }
        Command::Show {
            pokemon,
            force_kitty,
        } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            show(&config, &store, &pokemon, force_kitty)?;
        }
        Command::Palette { pokemon } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            let source = load_source(&store, &pokemon)?;
            let colors = sprite::bundled_palette(pokemon.id, store.game(), &store.variant())
                .unwrap_or_else(|| palette::extract(&source, &config.display.background));
            for color in colors {
                println!("{}", color.hex());
            }
        }
        Command::Icon { pokemon, output } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            let source = load_source(&store, &pokemon)?;
            let destination = output.unwrap_or_else(|| state_dir().join("Ghostty.icns"));
            icon::write(&source, &destination)?;
            println!("{}", destination.display());
        }
        Command::Sprite { pokemon } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            println!("{}", store.resolve(pokemon.id)?.display());
        }
        Command::Render {
            pokemon,
            output,
            pixels,
        } => {
            anyhow::ensure!(pixels >= 16, "render size must be at least 16 pixels");
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            let source = load_source(&store, &pokemon)?;
            let rendered = image_ops::render_square(&source, pixels, 3);
            image_ops::save_png(&rendered, &output)?;
            println!("{}", output.display());
        }
        Command::Bundle => println!("{}", sprite::bundle_profile()),
    }
    Ok(())
}

fn apply_overrides(config: &mut Config, cli: &Cli) {
    if !cli.game.is_empty() {
        config.sprites.game = if cli.game.len() == 1 {
            GameSelection::One(cli.game[0].clone())
        } else {
            GameSelection::Many(cli.game.clone())
        };
    }
    if let Some(variant) = &cli.variant {
        config.sprites.variant = variant.clone();
    }
    if cli.artwork {
        config.sprites.artwork = true;
    } else if cli.no_artwork {
        config.sprites.artwork = false;
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
        config.display.background = background.clone();
    }
    if cli.icon {
        config.icon.enabled = true;
    } else if cli.no_icon {
        config.icon.enabled = false;
    }
}

fn select<'a>(
    config: &'a Config,
    config_dir: &'a Path,
    selector: Option<&str>,
) -> Result<(SpriteStore<'a>, Pokemon)> {
    if config.sprites.game.is_pool() {
        if pokemon::is_random_selector(selector) {
            let store = SpriteStore::new(&config.sprites, config_dir, None)?;
            let pokemon = pokemon::resolve_available(selector, &config.sprites, |id| {
                store.has_bundled_sprite(id)
            })?;
            return Ok((store, pokemon));
        }

        let pokemon = pokemon::resolve(selector, &config.sprites)?;
        let store = SpriteStore::new(&config.sprites, config_dir, Some(pokemon.id))?;
        return Ok((store, pokemon));
    }

    let store = SpriteStore::new(&config.sprites, config_dir, None)?;
    let pokemon = pokemon::resolve(selector, &config.sprites)?;
    Ok((store, pokemon))
}

fn show(
    config: &Config,
    store: &SpriteStore<'_>,
    pokemon: &Pokemon,
    force_kitty: bool,
) -> Result<[palette::Color; palette::SIZE]> {
    let source = load_source(store, pokemon)?;
    let colors = sprite::bundled_palette(pokemon.id, store.game(), &store.variant())
        .unwrap_or_else(|| palette::extract(&source, &config.display.background));
    let png = if terminal::should_render_image(force_kitty) {
        let rendered = image_ops::render_square(&source, config.display.canvas_pixels(), 3);
        image_ops::encode_png(&rendered)?
    } else {
        Vec::new()
    };
    terminal::print_greeting(
        &png,
        pokemon,
        &store.label(),
        &colors,
        &config.display,
        force_kitty,
    )?;
    Ok(colors)
}

fn load_source(store: &SpriteStore<'_>, pokemon: &Pokemon) -> Result<image::RgbaImage> {
    let path = store.resolve(pokemon.id)?;
    image_ops::load_rgba(&path).with_context(|| format!("loading {}", pokemon.label()))
}

fn schedule_icon(id: u16, game: &str) -> Result<()> {
    let executable = std::env::current_exe().context("locating the pokefetch executable")?;
    ProcessCommand::new(executable)
        .args(["icon", &id.to_string()])
        .env("POKEFETCH_GAME_OVERRIDE", game)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting background icon generation")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_overrides, Cli};
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
}
