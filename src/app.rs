//! Command dispatch: one arm per subcommand, and the helpers they share.
//!
//! Read [`run`] first. Every subcommand follows the same three beats:
//!
//! 1. **Select** — decide which species and which sprite set ([`select`]).
//! 2. **Load** — get the pixels ([`load_source`]).
//! 3. **Do the thing** — draw, extract colors, write an icon, print a path.
//!
//! # Rust concepts on display
//!
//! - **Lifetimes on a returned struct**: [`select`] returns a
//!   [`SpriteStore<'a>`] that borrows the config it was built from. The `'a`
//!   annotation is the compiler's guarantee that the store cannot outlive the
//!   config, so there is no way to end up holding a dangling reference.
//! - **Exhaustive `match`**: adding a variant to
//!   [`Command`] without handling it here is a compile
//!   error, not a silently ignored subcommand.
//! - **Spawn without wait**: [`schedule_icon`] starts a child process and
//!   deliberately does not wait for it, because the greeting must not block.

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result};

use crate::cli::{apply_overrides, Cli, Command};
use crate::config::{state_dir, Config};
use crate::palette::{self, Color, SIZE as PALETTE_SIZE};
use crate::pokemon::{self, Pokemon};
use crate::sprite::{self, SpriteStore};
use crate::{icon, image_ops, terminal};

/// Runs one Pokefetch invocation from parsed arguments.
///
/// # Errors
///
/// Returns an error if the config file is malformed or fails validation, if a
/// sprite cannot be found or decoded, or if an output path cannot be written.
pub fn run(cli: Cli) -> Result<()> {
    let (mut config, config_dir) = Config::load()?;
    apply_overrides(&mut config, &cli);
    // Validate after merging, so an invalid flag combination is caught even
    // when the file on disk was fine on its own.
    config.validate()?;

    match cli.command() {
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
            for color in palette_for(&config, &store, &pokemon)? {
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

/// Chooses a species and the sprite set to draw it from.
///
/// Order matters, and it differs by case:
///
/// - **Random species from a pool of games**: pick the game first, then a
///   species that game actually has artwork for. Choosing the other way round
///   can strand you on a species missing from the chosen game.
/// - **Named species**: resolve the name first, then pick a game that has it.
/// - **Single fixed game**: the two are independent.
fn select<'a>(
    config: &'a Config,
    config_dir: &'a Path,
    selector: Option<&str>,
) -> Result<(SpriteStore<'a>, Pokemon)> {
    if !config.sprites.game.is_pool() {
        let store = SpriteStore::new(&config.sprites, config_dir, None)?;
        let pokemon = pokemon::resolve(selector, &config.sprites)?;
        return Ok((store, pokemon));
    }

    if pokemon::is_random_selector(selector) {
        let store = SpriteStore::new(&config.sprites, config_dir, None)?;
        let pokemon = pokemon::resolve_available(selector, &config.sprites, |id| {
            store.has_bundled_sprite(id)
        })?;
        return Ok((store, pokemon));
    }

    let pokemon = pokemon::resolve(selector, &config.sprites)?;
    let store = SpriteStore::new(&config.sprites, config_dir, Some(pokemon.id))?;
    Ok((store, pokemon))
}

/// Loads and prints one greeting.
fn show(
    config: &Config,
    store: &SpriteStore<'_>,
    pokemon: &Pokemon,
    force_kitty: bool,
) -> Result<()> {
    let colors = palette_for(config, store, pokemon)?;
    // Skip the encode entirely when nothing will display it. Rendering and
    // PNG-encoding a sprite is the most expensive step in a plain-text run.
    let png = if terminal::should_render_image(force_kitty) {
        let source = load_source(store, pokemon)?;
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
    )
}

/// Returns the sprite's palette, preferring the one baked in at build time.
///
/// Bundled sprites carry a precomputed palette so that a greeting never has to
/// run the color extractor. Falling back to [`palette::extract`] covers
/// downloaded and locally overridden sprites.
fn palette_for(
    config: &Config,
    store: &SpriteStore<'_>,
    pokemon: &Pokemon,
) -> Result<[Color; PALETTE_SIZE]> {
    if let Some(colors) = sprite::bundled_palette(pokemon.id, store.game(), &store.variant()) {
        return Ok(colors);
    }
    let source = load_source(store, pokemon)?;
    Ok(palette::extract(&source, &config.display.background))
}

/// Resolves a sprite path and decodes it into RGBA pixels.
fn load_source(store: &SpriteStore<'_>, pokemon: &Pokemon) -> Result<image::RgbaImage> {
    let path = store.resolve(pokemon.id)?;
    image_ops::load_rgba(&path).with_context(|| format!("loading {}", pokemon.label()))
}

/// Starts icon generation in a detached background process.
///
/// Building a seven-resolution `.icns` takes long enough to be felt at shell
/// startup, and the result is not needed until Ghostty next launches. So the
/// greeting re-invokes its own executable and returns immediately.
///
/// All three standard streams are set to [`Stdio::null`] so the child cannot
/// write into the terminal after the prompt has already been drawn.
fn schedule_icon(id: u16, game: &str) -> Result<()> {
    let executable = std::env::current_exe().context("locating the pokefetch executable")?;
    ProcessCommand::new(executable)
        .args(["icon", &id.to_string()])
        // The child re-reads config from scratch, so the chosen game has to be
        // passed explicitly or a random pool would re-roll a different one.
        .env("POKEFETCH_GAME_OVERRIDE", game)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting background icon generation")?;
    Ok(())
}

/// Path to the default generated Ghostty icon.
///
/// Exposed so that shell integration and documentation can name one location.
pub fn default_icon_path() -> PathBuf {
    state_dir().join("Ghostty.icns")
}
