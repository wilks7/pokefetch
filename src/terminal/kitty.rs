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
//! later chunk only says whether more follow.
//!
//! The keys this module sends:
//!
//! | Key     | Meaning                                                   |
//! |---------|-----------------------------------------------------------|
//! | `a=T`   | transmit **and** display in one action                      |
//! | `f=100` | payload is a PNG, so the terminal does the decoding         |
//! | `q=2`   | suppress replies, which would otherwise land in the shell    |
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

/// Largest base64 payload one escape sequence may carry, per the protocol.
const CHUNK_BYTES: usize = 4096;

/// Writes a PNG to `writer` as a sized, non-cursor-moving Kitty placement.
///
/// `columns` and `rows` describe the cell box the image is fitted into.
///
/// # Errors
///
/// Propagates any write failure from the underlying writer.
pub fn transmit(writer: &mut impl Write, png: &[u8], columns: u32, rows: u16) -> Result<()> {
    let encoded = STANDARD.encode(png);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK_BYTES).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        // `m` tells the terminal whether to keep reading. `usize::from(bool)`
        // is the idiomatic bool-to-number conversion: true becomes 1.
        let more = usize::from(index + 1 < chunks.len());
        if index == 0 {
            write!(
                writer,
                "\x1b_Ga=T,f=100,q=2,C=1,c={columns},r={rows},m={more};"
            )?;
        } else {
            write!(writer, "\x1b_Gm={more};")?;
        }
        writer.write_all(chunk)?;
        write!(writer, "\x1b\\")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{transmit, CHUNK_BYTES};

    #[test]
    fn writes_one_chunk_with_the_placement_keys() {
        let mut output = Vec::new();
        transmit(&mut output, b"tiny", 16, 8).unwrap();
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
        transmit(&mut output, &png, 16, 8).unwrap();
        let escaped = String::from_utf8(output).unwrap();
        assert!(escaped.contains("m=1;"), "early chunks must set m=1");
        assert!(escaped.contains("\x1b_Gm=0;"), "final chunk must set m=0");
        assert!(escaped.matches("\x1b_G").count() > 1, "expected chunking");
    }
}
