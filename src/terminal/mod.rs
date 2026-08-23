//! Drawing the greeting: image on the left, colored facts on the right.
//!
//! The submodules split this by concern, which is worth reading in order:
//!
//! - [`layout`] — pure row arithmetic, no I/O
//! - [`detect`] — which terminal is this, and can it draw?
//! - [`kitty`] — the escape-sequence protocol itself
//!
//! This file is the part that talks to the user: it assembles the information
//! lines and drives the other three.
//!
//! # Rust concepts on display
//!
//! - **`pub use` re-exports**: submodules keep the code organized, but callers
//!   write `terminal::should_render_image(..)` and never learn that `detect`
//!   exists. Module structure and public API are separate decisions.
//! - **Locking stdout once**: [`print_greeting`] takes a single lock and writes
//!   through it. Every `println!` otherwise locks and unlocks, which for a
//!   sprite-sized payload is measurable.
//! - **`iter().zip(..).cycle()`**: pairing each line with a palette color
//!   without indexing, and without caring which sequence is shorter.

mod detect;
mod kitty;
mod layout;

pub use detect::{is_local_ghostty, should_render_image, supports_kitty_graphics, terminal_label};
pub use layout::{greeting_layout, GreetingLayout};

use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result};

use crate::config::DisplayConfig;
use crate::palette::{Color, SIZE as PALETTE_SIZE};
use crate::pokemon::Pokemon;
use crate::system;

/// Prints the full greeting to stdout.
///
/// When the terminal cannot draw, `png` is ignored and the same information
/// lines are printed as plain text — the greeting degrades rather than fails.
///
/// # Errors
///
/// Returns an error if the line count cannot be laid out, or if writing to
/// stdout fails (a closed pipe, for instance).
pub fn print_greeting(
    png: &[u8],
    pokemon: &Pokemon,
    variant: &str,
    palette: &[Color; PALETTE_SIZE],
    display: &DisplayConfig,
    force_kitty: bool,
) -> Result<()> {
    // Locking once and reusing the handle avoids re-locking on every write.
    let mut output = io::stdout().lock();
    let lines = information_lines(pokemon, variant);

    if should_render_image(force_kitty) {
        let layout = greeting_layout(lines.len(), display)?;
        print_with_image(&mut output, png, &lines, palette, display, &layout)?;
    } else {
        print_plain(&mut output, &lines, palette, io::stdout().is_terminal())?;
    }
    output.flush().context("flushing greeting")
}

/// Draws the sprite, then rewinds the cursor and draws text beside it.
///
/// The sequence is: pad down to the sprite's row, transmit it, move the cursor
/// back *up* with `ESC[<n>A`, then indent each text line with `ESC[<n>C`.
/// Kitty placements do not move the cursor (`C=1`), so the text has to be
/// positioned explicitly.
fn print_with_image(
    output: &mut impl Write,
    png: &[u8],
    lines: &[String],
    palette: &[Color; PALETTE_SIZE],
    display: &DisplayConfig,
    layout: &GreetingLayout,
) -> Result<()> {
    for _ in 0..layout.image_offset {
        write!(output, "\r\n")?;
    }
    kitty::transmit(output, png, display.columns(), display.size)?;
    if layout.image_offset > 0 {
        write!(output, "\x1b[{}A", layout.image_offset)?;
    }
    for _ in 0..layout.text_offset {
        write!(output, "\r\n")?;
    }

    let indent = display.columns() + u32::from(display.gap);
    for (line, color) in pair_with_palette(lines, palette) {
        write!(
            output,
            "\r\x1b[{indent}C\x1b[38;2;{};{};{}m{line}\x1b[0m\r\n",
            color.red, color.green, color.blue
        )?;
    }

    // Pad out so the shell prompt lands below the sprite, not on top of it.
    let occupied = layout.text_offset + lines.len();
    for _ in occupied..layout.height {
        write!(output, "\r\n")?;
    }
    Ok(())
}

/// Prints the information lines with no image.
///
/// `colored` is false when stdout is redirected, so piping the greeting into a
/// file yields clean text rather than escape codes.
fn print_plain(
    output: &mut impl Write,
    lines: &[String],
    palette: &[Color; PALETTE_SIZE],
    colored: bool,
) -> Result<()> {
    for (line, color) in pair_with_palette(lines, palette) {
        if colored {
            writeln!(
                output,
                "\x1b[38;2;{};{};{}m{line}\x1b[0m",
                color.red, color.green, color.blue
            )?;
        } else {
            writeln!(output, "{line}")?;
        }
    }
    Ok(())
}

/// Pairs each line with its palette color.
///
/// `cycle` makes the palette repeat, so this is correct for any line count.
/// `zip` stops at the shorter sequence, and since `cycle` never ends, the
/// line count always wins — no bounds check needed.
fn pair_with_palette<'a>(
    lines: &'a [String],
    palette: &'a [Color; PALETTE_SIZE],
) -> impl Iterator<Item = (&'a String, &'a Color)> {
    lines.iter().zip(palette.iter().cycle())
}

/// Builds the five information lines shown beside the sprite.
///
/// Every lookup has a fallback, so the greeting prints on a machine with no
/// `USER`, no resolvable hostname, and no recognizable shell.
///
/// The palette is eight colors wide and the layout accepts one through eight
/// lines, so rows can be added here without touching the renderer.
fn information_lines(pokemon: &Pokemon, variant: &str) -> Vec<String> {
    let user = std::env::var("USER")
        .ok()
        .filter(|value| !value.is_empty())
        .map_or_else(|| "Trainer".to_string(), |value| capitalize(&value));
    let host = hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    // SHELL is an absolute path; only the final component is worth showing.
    let shell = std::env::var("SHELL")
        .ok()
        .and_then(|value| value.rsplit('/').next().map(str::to_owned))
        .map_or_else(|| "Shell".to_string(), |value| capitalize(&value));
    let snapshot = system::snapshot();

    vec![
        format!("{user} @ {host}"),
        snapshot.system,
        snapshot.hardware,
        format!("{shell} · {} · {}", terminal_label(), snapshot.packages),
        format!("{} · {variant}", pokemon.label()),
    ]
}

/// Uppercases the first character, leaving the rest alone.
///
/// Rust has no `str::capitalize` because "the first character" is ambiguous
/// once you leave ASCII. `to_uppercase` yields an *iterator* of chars for the
/// same reason, so the result is collected rather than pushed.
pub(crate) fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{capitalize, pair_with_palette, print_plain};
    use crate::palette::{Color, SIZE};

    /// A palette whose red channel encodes its index, so tests can assert
    /// which slot a line was colored from.
    fn test_palette() -> [Color; SIZE] {
        std::array::from_fn(|index| Color::rgb(u8::try_from(index).unwrap_or(u8::MAX), 0, 0))
    }

    #[test]
    fn capitalizes_only_the_first_character() {
        assert_eq!(capitalize("fish"), "Fish");
        assert_eq!(capitalize("zsh"), "Zsh");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("aBC"), "ABC");
    }

    #[test]
    fn cycles_the_palette_past_its_width() {
        let lines = (0..10).map(|n| n.to_string()).collect::<Vec<_>>();
        let palette = test_palette();
        let paired = pair_with_palette(&lines, &palette).collect::<Vec<_>>();
        assert_eq!(paired.len(), 10, "one pair per line, never per color");
        assert_eq!(paired[8].1.red, 0, "palette wraps around");
    }

    #[test]
    fn omits_escape_codes_when_stdout_is_not_a_terminal() {
        let lines = vec!["Trainer @ studio".to_string()];
        let mut output = Vec::new();
        print_plain(&mut output, &lines, &test_palette(), false).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "Trainer @ studio\n");
    }

    #[test]
    fn emits_truecolor_escapes_when_stdout_is_a_terminal() {
        let lines = vec!["Trainer @ studio".to_string()];
        let mut output = Vec::new();
        print_plain(&mut output, &lines, &test_palette(), true).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("\x1b[38;2;0;0;0m"));
        assert!(text.ends_with("\x1b[0m\n"));
    }
}
