//! Deciding how many blank rows go above the image and above the text.
//!
//! The greeting draws two columns of different heights — a sprite that is
//! `display.size` rows tall, and one text line per information row. Whichever
//! is shorter gets padded so the two read as one block.
//!
//! This module is pure arithmetic: no I/O, no environment, no escape codes.
//! That is deliberate, and it is why [`greeting_layout`] is the most heavily
//! tested function in the crate — pure functions are trivial to test.
//!
//! ```text
//!   size = 8, 5 text lines, centered
//!
//!   row 0  ┌────────┐
//!   row 1  │        │  Trainer @ studio     <- text_offset = 1
//!   row 2  │ sprite │  macOS 15.3 · M2
//!   row 3  │        │  8C CPU · 10C GPU
//!   row 4  │        │  Fish · Ghostty
//!   row 5  │        │  #025 Pikachu
//!   row 6  │        │
//!   row 7  └────────┘
//! ```

use anyhow::Result;

use crate::config::{Alignment, DisplayConfig};
use crate::palette::SIZE as PALETTE_SIZE;

/// Where the sprite and the text start, and how tall the whole block is.
///
/// All three fields are row counts, not pixels. Kitty graphics places images
/// on the text grid, so the greeting only ever reasons in terminal rows.
#[derive(Debug, Eq, PartialEq)]
pub struct GreetingLayout {
    /// Total rows the greeting occupies.
    pub height: usize,
    /// Blank rows printed before the sprite.
    pub image_offset: usize,
    /// Blank rows printed before the first text line.
    pub text_offset: usize,
}

/// Computes the layout for `line_count` information lines.
///
/// Pokefetch never invents filler text: if there are fewer lines than sprite
/// rows, the text is offset rather than padded out.
///
/// ```
/// # use pokefetch::config::DisplayConfig;
/// # use pokefetch::terminal::greeting_layout;
/// // A default 8-row sprite beside 5 text lines centers the text.
/// let layout = greeting_layout(5, &DisplayConfig::default()).unwrap();
/// assert_eq!(layout.height, 8);
/// assert_eq!(layout.image_offset, 0);
/// assert_eq!(layout.text_offset, 1);
/// ```
///
/// # Errors
///
/// Returns an error unless `line_count` is between 1 and the palette width,
/// since each line takes its color from a different palette slot.
pub fn greeting_layout(line_count: usize, display: &DisplayConfig) -> Result<GreetingLayout> {
    anyhow::ensure!(
        (1..=PALETTE_SIZE).contains(&line_count),
        "greeting needs between 1 and {PALETTE_SIZE} information lines"
    );
    let image_height = usize::from(display.size);
    let height = image_height.max(line_count);
    // Integer division truncates, so an odd amount of slack puts the extra row
    // below the content. That is the conventional choice and it keeps the
    // function free of rounding configuration.
    let (image_offset, text_offset) = match display.alignment {
        Alignment::Top => (0, 0),
        Alignment::Center => ((height - image_height) / 2, (height - line_count) / 2),
    };
    Ok(GreetingLayout {
        height,
        image_offset,
        text_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::{greeting_layout, GreetingLayout};
    use crate::config::{Alignment, DisplayConfig};

    #[test]
    fn supports_between_one_and_eight_information_lines() {
        let display = DisplayConfig::default();
        assert!(greeting_layout(1, &display).is_ok());
        assert!(greeting_layout(8, &display).is_ok());
        assert!(greeting_layout(0, &display).is_err());
        assert!(greeting_layout(9, &display).is_err());
    }

    #[test]
    fn centers_shorter_text_or_image_by_terminal_row() {
        let display = DisplayConfig::default();
        assert_eq!(
            greeting_layout(5, &display).unwrap(),
            GreetingLayout {
                height: 8,
                image_offset: 0,
                text_offset: 1,
            }
        );

        let display = DisplayConfig {
            size: 2,
            ..DisplayConfig::default()
        };
        assert_eq!(
            greeting_layout(6, &display).unwrap(),
            GreetingLayout {
                height: 6,
                image_offset: 2,
                text_offset: 0,
            }
        );
    }

    #[test]
    fn top_alignment_never_adds_an_offset() {
        let display = DisplayConfig {
            size: 2,
            alignment: Alignment::Top,
            ..DisplayConfig::default()
        };
        assert_eq!(
            greeting_layout(6, &display).unwrap(),
            GreetingLayout {
                height: 6,
                image_offset: 0,
                text_offset: 0,
            }
        );
    }
}
