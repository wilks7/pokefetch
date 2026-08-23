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
use clap::{Parser, Subcommand};

use crate::config::{state_dir, Config};
use crate::pokemon::Pokemon;
use crate::sprite::SpriteStore;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show a random Pokemon greeting and prepare Ghostty's next icon.
    Greet {
        pokemon: Option<String>,
        #[arg(long)]
        force_kitty: bool,
        #[arg(long)]
        no_icon: bool,
    },
    /// Show one Pokemon without changing Ghostty's icon.
    Show {
        pokemon: Option<String>,
        #[arg(long)]
        force_kitty: bool,
    },
    /// Print the four terminal colors extracted from a sprite.
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
        size: u32,
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
    let (config, config_dir) = Config::load()?;
    config.validate()?;

    match cli.command.unwrap_or(Command::Greet {
        pokemon: None,
        force_kitty: false,
        no_icon: false,
    }) {
        Command::Greet {
            pokemon,
            force_kitty,
            no_icon,
        } => {
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            show(&config, &store, &pokemon, force_kitty)?;
            if config.icon.enabled && !no_icon && terminal::is_local_ghostty() {
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
            size,
        } => {
            anyhow::ensure!(size >= 16, "render size must be at least 16 pixels");
            let (store, pokemon) = select(&config, &config_dir, pokemon.as_deref())?;
            let source = load_source(&store, &pokemon)?;
            let rendered = image_ops::render_square(&source, size, 3);
            image_ops::save_png(&rendered, &output)?;
            println!("{}", output.display());
        }
        Command::Bundle => println!("{}", sprite::bundle_profile()),
    }
    Ok(())
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
        let rendered = image_ops::render_square(&source, config.display.canvas_pixels, 3);
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
