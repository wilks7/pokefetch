//! Working out which terminal we are running inside, and what it can draw.
//!
//! There is no portable "can you show me a picture?" query, so this module
//! reads environment variables that terminals set about themselves. Each
//! signal is unreliable in a different way, which is why there are several:
//!
//! - `TERM_PROGRAM` names the application, but SSH does not forward it.
//! - `TERM` survives SSH because the pseudo-terminal carries it, so
//!   `xterm-ghostty` is the only hint a remote session gets.
//! - `KITTY_WINDOW_ID` is set only by Kitty itself.
//!
//! # Rust concepts on display
//!
//! - **Testable boundaries**: [`kitty_terminal_name`] takes the environment as
//!   *parameters* instead of reading it directly. That one choice is what makes
//!   the detection logic unit-testable without mutating process state — the
//!   public wrappers do the reading, the private function does the deciding.
//! - **`Option<&str>` vs `&str`**: absent and empty are different states here,
//!   and the type keeps them apart.

use std::io::{self, IsTerminal};

/// Reports whether the current terminal understands Kitty graphics escapes.
pub fn supports_kitty_graphics() -> bool {
    current_terminal_name().is_some()
}

/// Reports whether the greeting should transmit an image at all.
///
/// Two conditions must hold: stdout is a terminal (not a pipe or a file, where
/// escape codes would be garbage), and that terminal can draw. `force_kitty`
/// overrides both for terminals that support the protocol without saying so.
pub fn should_render_image(force_kitty: bool) -> bool {
    force_kitty || (io::stdout().is_terminal() && supports_kitty_graphics())
}

/// Reports whether this is a local Ghostty session, where icon swaps apply.
///
/// Over SSH the icon would be written on the wrong machine, so the presence of
/// either SSH variable disqualifies the session.
pub fn is_local_ghostty() -> bool {
    std::env::var("TERM_PROGRAM").as_deref() == Ok("ghostty")
        && std::env::var_os("SSH_CONNECTION").is_none()
        && std::env::var_os("SSH_TTY").is_none()
}

/// Returns a display name for the terminal, e.g. `Ghostty`.
///
/// Falls back to a capitalized `TERM_PROGRAM`, then to a generic label, so the
/// greeting always has something to print.
pub fn terminal_label() -> String {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    current_terminal_name()
        .map(str::to_owned)
        .or_else(|| {
            term_program
                .filter(|value| !value.is_empty())
                .map(|value| super::capitalize(&value))
        })
        .unwrap_or_else(|| "Terminal".to_string())
}

/// Reads the environment and asks [`kitty_terminal_name`] to decide.
fn current_terminal_name() -> Option<&'static str> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let term = std::env::var("TERM").ok();
    kitty_terminal_name(
        term_program.as_deref(),
        term.as_deref(),
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
    )
}

/// Identifies a graphics-capable terminal from three environment signals.
///
/// Returns [`None`] for terminals that cannot draw images. Taking the
/// environment as arguments rather than reading it makes this pure, and the
/// tests below exercise the SSH case that would otherwise need a real
/// remote session to reproduce.
fn kitty_terminal_name(
    term_program: Option<&str>,
    term: Option<&str>,
    kitty_window: bool,
) -> Option<&'static str> {
    // Checked first: TERM_PROGRAM is the most specific signal when present.
    match term_program {
        Some("ghostty") => return Some("Ghostty"),
        Some("kitty") => return Some("Kitty"),
        Some("WezTerm") => return Some("WezTerm"),
        _ => {}
    }
    if kitty_window {
        return Some("Kitty");
    }
    // Reached over SSH, which drops TERM_PROGRAM but carries TERM.
    match term {
        Some("xterm-ghostty") => Some("Ghostty"),
        Some("xterm-kitty") => Some("Kitty"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::kitty_terminal_name;

    #[test]
    fn prefers_the_terminal_program_when_it_is_present() {
        assert_eq!(
            kitty_terminal_name(Some("ghostty"), Some("xterm-256color"), false),
            Some("Ghostty")
        );
        assert_eq!(
            kitty_terminal_name(Some("WezTerm"), None, false),
            Some("WezTerm")
        );
    }

    #[test]
    fn recognizes_graphics_terminals_across_ssh() {
        // SSH forwards TERM but not TERM_PROGRAM, so TERM is the only signal.
        assert_eq!(
            kitty_terminal_name(None, Some("xterm-ghostty"), false),
            Some("Ghostty")
        );
        assert_eq!(
            kitty_terminal_name(None, Some("xterm-kitty"), false),
            Some("Kitty")
        );
        assert_eq!(
            kitty_terminal_name(None, Some("xterm-256color"), false),
            None
        );
    }

    #[test]
    fn falls_back_to_the_kitty_window_marker() {
        assert_eq!(kitty_terminal_name(None, None, true), Some("Kitty"));
        assert_eq!(kitty_terminal_name(None, None, false), None);
    }
}
