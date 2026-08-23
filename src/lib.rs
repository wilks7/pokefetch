//! Pokefetch renders a Pokemon sprite and some machine facts in your terminal.
//!
//! # Reading this crate
//!
//! This repository doubles as a worked example of an ordinary Rust CLI. If you
//! are learning the language, `docs/tour/` walks the modules in a deliberate
//! order. If you would rather browse the API, run:
//!
//! ```sh
//! cargo doc --open
//! ```
//!
//! # How a greeting happens
//!
//! ```text
//!   config.toml + CLI flags   ->  config   (what should be drawn)
//!                                   |
//!                   pokemon  <------+  which species?
//!                                   |
//!                   sprite   <------+  where are its pixels?
//!                                   |
//!                image_ops   <------+  crop, scale, encode
//!                                   |
//!                  palette   <------+  eight colors from those pixels
//!                                   |
//!                 terminal   <------+  draw image + colored text
//! ```
//!
//! # Module map
//!
//! | Module        | Responsibility                                        |
//! |---------------|-------------------------------------------------------|
//! | [`cli`]       | Command-line surface, and how flags override the file  |
//! | [`app`]       | Dispatch: one arm per subcommand                       |
//! | [`config`]    | The TOML file, defaults, and validation                |
//! | [`pokemon`]   | Selector (`pikachu`, `25`, `random`) to species        |
//! | [`sprite`]    | Finding pixels: bundled, cached, local, or downloaded  |
//! | [`image_ops`] | Cropping, nearest-neighbor scaling, PNG encoding       |
//! | [`palette`]   | Extracting eight terminal colors from a sprite         |
//! | [`terminal`]  | Terminal detection, Kitty graphics, layout             |
//! | [`system`]    | Cached machine facts (the shell-startup budget)        |
//! | [`icon`]      | Writing a macOS `.icns` for Ghostty                    |
//!
//! # Why a library plus a binary?
//!
//! `src/main.rs` is deliberately tiny. Everything real lives here in the
//! library, which buys three things a binary-only crate cannot have:
//!
//! 1. `cargo doc` renders these pages.
//! 2. Examples in doc comments are compiled and run by `cargo test`, so the
//!    documentation cannot drift out of date without failing the build.
//! 3. `tests/` can exercise the crate as an outside caller would.
//!
//! # A note on error handling
//!
//! This crate uses [`anyhow`] throughout rather than defining its own error
//! enum. That is the right trade for an application: errors are shown to a
//! person, not matched on by a caller. A *library* other code depends on would
//! usually define concrete error types instead, often with `thiserror`.

// Warn on public items that lack documentation. Combined with the lint
// configuration in Cargo.toml, this keeps the teaching value from decaying.
#![warn(missing_docs)]

pub mod app;
pub mod cli;
pub mod config;
pub mod icon;
pub mod image_ops;
pub mod palette;
pub mod pokemon;
pub mod sprite;
pub mod system;
pub mod terminal;
