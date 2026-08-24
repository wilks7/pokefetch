# Development and verification

Pokefetch is a working tool first and a learn-by-example Rust codebase second.
The crate requires Rust 1.85 or newer.

## Run from a checkout

```sh
cargo run -- show pikachu
cargo run -- --game emerald --size 8 show rayquaza
```

A normal build embeds no artwork and downloads missing sprites into the runtime
cache. To exercise the same bundle shipped in releases:

```sh
POKEFETCH_BUNDLE=retro-master cargo run --features bundle-assets -- show pikachu
```

## Required checks

Before committing Rust changes, run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
POKEFETCH_BUNDLE=retro-master cargo test --features bundle-assets
```

The final command is required because it is the only test build that validates
the complete release asset manifest. CI treats warnings as errors.

Generate the API documentation with:

```sh
cargo doc --open
```

Public-item examples are compiled and run by `cargo test`.

## Bootstrap contract

The dry-run-first bootstrap is the stable integration point used by the
dotfiles repository. It wraps `cargo install` rather than implementing a
separate packaging system.

```sh
bootstrap/bootstrap
bootstrap/bootstrap --apply
bootstrap/doctor
```

Choose a different bundle or install root when needed:

```sh
bootstrap/bootstrap --bundle crystal-full --root /tmp/pokefetch --apply
bootstrap/doctor --bundle crystal-full --root /tmp/pokefetch
```

## Codebase tour

[`docs/tour/`](tour/README.md) walks through ownership, errors, traits,
iterators, modules, testing, and the build script for readers learning Rust.
Every module also begins with a `//!` comment explaining what it does and which
Rust concepts it demonstrates.

The main crate is in `src/lib.rs`; `src/main.rs` is intentionally a thin binary
over it. The asset importer lives in `src/bin/pokefetch-assets.rs`, integration
tests live in `tests/`, and shell integration lives in `shell/`.

For bundle generation, provenance, and asset imports, see
[Assets and distribution](assets-and-distribution.md).
