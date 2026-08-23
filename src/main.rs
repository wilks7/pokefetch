//! The `pokefetch` executable.
//!
//! This file is intentionally about thirty lines. Its whole job is to parse
//! arguments, hand off to the library, and turn an error into an exit code.
//! Everything worth reading lives in [`pokefetch`] — start at `src/lib.rs`.

use clap::Parser;

use pokefetch::cli::Cli;

/// Entry point.
///
/// `main` returns `()` rather than `Result` so that errors print with
/// `anyhow`'s `{:#}` formatting — which includes the chain of `.context()`
/// messages on one line — instead of the `Debug` output Rust would produce for
/// a returned `Err`. Compare:
///
/// ```text
/// returning Result:  Error: loading #025 Pikachu \n Caused by: ...
/// this function:     pokefetch: loading #025 Pikachu: decoding /path: ...
/// ```
fn main() {
    if let Err(error) = pokefetch::app::run(Cli::parse()) {
        eprintln!("pokefetch: {error:#}");
        std::process::exit(1);
    }
}
