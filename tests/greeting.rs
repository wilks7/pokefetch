//! Integration tests: exercising Pokefetch the way an outside caller would.
//!
//! Files in `tests/` are compiled as separate crates that link against the
//! library, so they can only reach `pub` items. That is the point — a unit test
//! inside a module can poke at private helpers, but these can only use the API
//! the crate actually offers. If something here does not compile, the public
//! surface is wrong.
//!
//! Nothing in this file touches the network or the user's real config, so the
//! suite stays fast and deterministic.

use image::{Rgba, RgbaImage};
use pokefetch::config::{Alignment, Config, DisplayConfig};
use pokefetch::palette::{self, SIZE};
use pokefetch::pokemon;
use pokefetch::terminal::greeting_layout;

/// Builds a small sprite with a few distinct colors.
fn sample_sprite() -> RgbaImage {
    let mut sprite = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
    for x in 0..16 {
        for y in 0..8 {
            sprite.put_pixel(x, y, Rgba([240, 200, 40, 255]));
        }
    }
    for x in 4..12 {
        sprite.put_pixel(x, 10, Rgba([200, 40, 40, 255]));
    }
    sprite
}

#[test]
fn a_default_configuration_is_immediately_valid() {
    // The promise the README makes: Pokefetch runs with no config file.
    let config = Config::default();
    assert!(config.validate().is_ok());
}

#[test]
fn the_full_selection_pipeline_produces_a_drawable_greeting() {
    // Walks the same path a real greeting takes, minus the I/O:
    // config -> species -> palette -> layout.
    let config = Config::default();
    config.validate().unwrap();

    let pokemon = pokemon::resolve(Some("pikachu"), &config.sprites).unwrap();
    assert_eq!(pokemon.id, 25);
    assert_eq!(pokemon.label(), "#025 Pikachu");

    let colors = palette::extract(&sample_sprite(), &config.display.background);
    assert_eq!(colors.len(), SIZE);

    // Five information lines is what the greeting currently emits.
    let layout = greeting_layout(5, &config.display).unwrap();
    assert_eq!(layout.height, usize::from(config.display.size));
    assert!(layout.text_offset + 5 <= layout.height);
}

#[test]
fn every_supported_line_count_lays_out_within_the_palette() {
    let display = DisplayConfig::default();
    for lines in 1..=SIZE {
        let layout = greeting_layout(lines, &display).unwrap();
        assert!(
            layout.text_offset + lines <= layout.height,
            "{lines} lines overflowed the block"
        );
    }
    assert!(greeting_layout(SIZE + 1, &display).is_err());
}

#[test]
fn top_alignment_and_centering_agree_when_heights_match() {
    // With as many lines as rows there is no slack, so alignment is a no-op.
    let display = DisplayConfig {
        size: 5,
        alignment: Alignment::Top,
        ..DisplayConfig::default()
    };
    let top = greeting_layout(5, &display).unwrap();

    let display = DisplayConfig {
        alignment: Alignment::Center,
        ..display
    };
    let centered = greeting_layout(5, &display).unwrap();

    assert_eq!(top, centered);
}

#[test]
fn a_configured_size_drives_both_columns_and_canvas() {
    for size in [1, 8, 32] {
        let display = DisplayConfig {
            size,
            ..DisplayConfig::default()
        };
        assert_eq!(display.columns(), u32::from(size) * 2);
        assert_eq!(display.canvas_pixels(), u32::from(size) * 32);
    }
}

#[test]
fn palette_colors_are_all_printable_hex() {
    let colors = palette::extract(&sample_sprite(), "#222436");
    for color in colors {
        let hex = color.hex();
        assert_eq!(hex.len(), 7, "expected #RRGGBB, got {hex}");
        assert!(hex.starts_with('#'));
        assert!(hex[1..].chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn a_species_outside_the_configured_range_is_still_nameable() {
    // Range limits random selection; it does not limit explicit requests.
    let config = Config::default(); // range_end = 151
    let mew_two = pokemon::resolve(Some("250"), &config.sprites).unwrap();
    assert_eq!(mew_two.name, "Ho-Oh");
}
