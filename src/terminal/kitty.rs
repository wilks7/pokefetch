//! The Kitty graphics protocol, as much of it as Pokefetch needs.
//!
//! Kitty's protocol (which Ghostty and `WezTerm` also implement) sends image
//! data inline as an escape sequence. The shape is:
//!
//! ```text
//! ESC _ G <comma-separated keys> ; <base64 payload> ESC \
//! ```
//!
//! Payloads are chunked because terminals are not required to buffer an
//! unbounded escape sequence. Only the first chunk carries the keys; every
//! later chunk says whether more follow. Animation frame continuations also
//! repeat `a=f`, as required by the protocol.
//!
//! The keys this module sends:
//!
//! | Key     | Meaning                                                   |
//! |---------|-----------------------------------------------------------|
//! | `a=T`   | transmit **and** display in one action                      |
//! | `a=f`   | transmit another frame for an existing image                |
//! | `a=a`   | configure or start a terminal-driven animation              |
//! | `f=100` | payload is a PNG, so the terminal does the decoding         |
//! | `q=2`   | suppress replies, which would otherwise land in the shell    |
//! | `I`     | non-unique image number used to associate animation frames   |
//! | `z`     | delay before advancing from one animation frame              |
//! | `C=1`   | do not move the cursor, so text can be drawn alongside       |
//! | `c`,`r` | placement size in **columns and rows**, not pixels           |
//! | `m`     | 1 if more chunks follow, 0 for the last one                  |
//!
//! Sizing in rows rather than pixels is the reason a sprite stays visually
//! consistent when the terminal font size changes.
//!
//! # Rust concepts on display
//!
//! - **`impl Write` over a concrete type**: [`transmit`] accepts any writer, so
//!   tests can pass a `Vec<u8>` and assert on the exact bytes emitted instead
//!   of trying to inspect a real terminal.

use std::io::Write;

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};

use super::ImageFrame;

/// Largest base64 payload one escape sequence may carry, per the protocol.
const CHUNK_BYTES: usize = 4096;

/// Application image number (`POKE` in ASCII).
///
/// Image numbers are intentionally non-unique: the terminal creates a fresh
/// image and lets subsequent commands target the newest image with this
/// number. That avoids colliding with another terminal program's image ID.
const IMAGE_NUMBER: u32 = 0x504F_4B45;

/// Writes PNG frames as a sized, non-cursor-moving Kitty placement.
///
/// `columns` and `rows` describe the cell box the image is fitted into. A
/// one-element slice uses the ordinary still-image path. Multiple frames are
/// uploaded once and played by the terminal, so Pokefetch can exit normally.
///
/// # Errors
///
/// Propagates any write failure from the underlying writer.
pub fn transmit(
    writer: &mut impl Write,
    frames: &[ImageFrame],
    columns: u32,
    rows: u16,
) -> Result<()> {
    let Some(first) = frames.first() else {
        return Ok(());
    };

    if frames.len() == 1 {
        let control = format!("a=T,f=100,q=2,C=1,c={columns},r={rows}");
        return transmit_payload(writer, &first.png, &control, "");
    }

    let control = format!("a=T,f=100,q=2,C=1,c={columns},r={rows},I={IMAGE_NUMBER}");
    transmit_payload(writer, &first.png, &control, "")?;

    for frame in &frames[1..] {
        let control = format!(
            "a=f,f=100,q=2,I={IMAGE_NUMBER},X=1,z={}",
            frame.delay_ms.max(1)
        );
        transmit_payload(writer, &frame.png, &control, "a=f,")?;
    }

    // The root frame is created by the ordinary image transmission above, so
    // its delay has to be assigned separately before playback starts.
    write!(
        writer,
        "\x1b_Ga=a,q=2,I={IMAGE_NUMBER},r=1,z={}\x1b\\",
        first.delay_ms.max(1)
    )?;
    write!(writer, "\x1b_Ga=a,q=2,I={IMAGE_NUMBER},s=3,v=1\x1b\\")?;
    Ok(())
}

/// Base64-encodes and chunks one Kitty graphics payload.
fn transmit_payload(
    writer: &mut impl Write,
    payload: &[u8],
    first_control: &str,
    continuation_control: &str,
) -> Result<()> {
    let encoded = STANDARD.encode(payload);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK_BYTES).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        // `m` tells the terminal whether to keep reading. `usize::from(bool)`
        // is the idiomatic bool-to-number conversion: true becomes 1.
        let more = usize::from(index + 1 < chunks.len());
        if index == 0 {
            write!(writer, "\x1b_G{first_control},m={more};")?;
        } else {
            write!(writer, "\x1b_G{continuation_control}m={more};")?;
        }
        writer.write_all(chunk)?;
        write!(writer, "\x1b\\")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{transmit, CHUNK_BYTES, IMAGE_NUMBER};
    use crate::terminal::ImageFrame;

    fn still(png: Vec<u8>) -> Vec<ImageFrame> {
        vec![ImageFrame { png, delay_ms: 0 }]
    }

    #[test]
    fn writes_one_chunk_with_the_placement_keys() {
        let mut output = Vec::new();
        transmit(&mut output, &still(b"tiny".to_vec()), 16, 8).unwrap();
        let escaped = String::from_utf8(output).unwrap();
        assert!(escaped.starts_with("\x1b_Ga=T,f=100,q=2,C=1,c=16,r=8,m=0;"));
        assert!(escaped.ends_with("\x1b\\"));
    }

    #[test]
    fn splits_large_payloads_and_marks_continuation() {
        // Three base64 characters encode from every two source bytes, so this
        // comfortably exceeds one chunk and forces a continuation.
        let png = vec![0_u8; CHUNK_BYTES * 2];
        let mut output = Vec::new();
        transmit(&mut output, &still(png), 16, 8).unwrap();
        let escaped = String::from_utf8(output).unwrap();
        assert!(escaped.contains("m=1;"), "early chunks must set m=1");
        assert!(escaped.contains("\x1b_Gm=0;"), "final chunk must set m=0");
        assert!(escaped.matches("\x1b_G").count() > 1, "expected chunking");
    }

    #[test]
    fn uploads_and_starts_a_terminal_driven_animation() {
        let frames = vec![
            ImageFrame {
                png: b"first".to_vec(),
                delay_ms: 80,
            },
            ImageFrame {
                png: b"second".to_vec(),
                delay_ms: 120,
            },
        ];
        let mut output = Vec::new();

        transmit(&mut output, &frames, 16, 8).unwrap();

        let escaped = String::from_utf8(output).unwrap();
        assert!(escaped.starts_with(&format!(
            "\x1b_Ga=T,f=100,q=2,C=1,c=16,r=8,I={IMAGE_NUMBER},m=0;"
        )));
        assert!(escaped.contains(&format!(
            "\x1b_Ga=f,f=100,q=2,I={IMAGE_NUMBER},X=1,z=120,m=0;"
        )));
        assert!(escaped.contains(&format!("\x1b_Ga=a,q=2,I={IMAGE_NUMBER},r=1,z=80\x1b\\")));
        assert!(escaped.ends_with(&format!("\x1b_Ga=a,q=2,I={IMAGE_NUMBER},s=3,v=1\x1b\\")));
    }

    #[test]
    fn repeats_the_frame_action_on_animation_continuations() {
        let frames = vec![
            ImageFrame {
                png: b"root".to_vec(),
                delay_ms: 40,
            },
            ImageFrame {
                png: vec![0_u8; CHUNK_BYTES * 2],
                delay_ms: 40,
            },
        ];
        let mut output = Vec::new();

        transmit(&mut output, &frames, 16, 8).unwrap();

        let escaped = String::from_utf8(output).unwrap();
        assert!(escaped.contains("\x1b_Ga=f,m=0;"));
    }
}
