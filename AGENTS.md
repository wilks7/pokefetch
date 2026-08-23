# Pokefetch contributor instructions

- Treat interactive shell startup latency as a primary product constraint.
  Normal greetings must not synchronously access the network, and expensive
  system or package-manager probes must use durable local caches.
- Preserve source sprite pixels. Scale rendered sprites with nearest-neighbor
  filtering and do not hand-edit imported upstream assets.
- Keep active greeting bundles to transparent, front-facing artwork. Back,
  shiny, gray, and opaque source variants are intentionally shelved until
  explicitly revisited.
- Model sprite sets by game/version rather than by a Pokemon's debut
  generation. Selection policy (one game set, introduced-only, or debut set)
  is separate from bundle contents.
- Pin and record upstream asset provenance when adding or regenerating sprite
  sets. Derive terminal palettes during the asset/build pipeline rather than
  during the bundled greeting path.
- Keep the canonical palette at eight colors. Greeting renderers may consume
  one through eight lines; the current system profile intentionally uses five.
- Before committing Rust changes, run `cargo fmt --check`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, and the bundled build that
  releases ship: `POKEFETCH_BUNDLE=retro-master cargo test --features
  bundle-assets`.

## This repository is also a Rust teaching example

Pokefetch is a working tool first. It is additionally maintained as a
learn-by-example codebase, which adds a few obligations:

- Every module opens with a `//!` comment stating what it does and which Rust
  concepts it demonstrates. Keep these accurate when the module changes.
- Every public item carries a doc comment; `missing_docs` is a warning and CI
  treats warnings as errors.
- Prefer the idiomatic construct over the clever one, and comment the choice
  when a reader might reasonably expect the other. Explain code constraints,
  not history.
- Examples in doc comments are compiled and run by `cargo test`. Add them where
  a signature alone is ambiguous.
- `docs/tour/` walks the codebase for a Rust beginner. If you move, rename, or
  split a module, update the chapters that reference it — they cite files and
  function names by hand.
- Lints live in `Cargo.toml` under `[lints]` at `clippy::pedantic`. Prefer
  fixing a warning; when silencing one, do it at the narrowest scope with a
  comment saying why.

## Layout

- `src/lib.rs` holds the crate; `src/main.rs` is a thin binary over it. Keep it
  thin — logic belongs in the library so it can be documented and tested.
- `src/bin/pokefetch-assets.rs` is the asset import tool and may depend on the
  library. `build.rs` may not, and shares `src/palette.rs` via `#[path]`.
- `tests/` holds integration tests that see only the public API.
- `shell/` holds shell integration. `docs/ghostty.md` covers the terminal side.

## Handoff

This file is the provider-neutral project brief for Claude, Codex, OpenCode,
and other agents. Read `README.md`, `docs/tour/README.md`,
`docs/assets-and-distribution.md`, and the manifests before changing bundle
architecture.

Current direction:

- Keep the repository simple. The bootstrap and doctor are narrow wrappers
  around `cargo install` and a no-config greeting because the dotfiles
  bootstrap depends on that contract. Elaborate multi-flavor packaging and
  provenance machinery were deliberately removed; do not expand the wrappers
  back into a release toolkit without an explicit decision.
- Hosted validation is one manual-only workflow that runs formatting, Clippy,
  tests, docs, and the bootstrap against both the default build and
  `retro-master`. The separate manual release workflow packages an existing
  version tag into a draft release; neither workflow runs automatically on a
  push or pull request.
- There is one bundle feature, `bundle-assets`, driven by `POKEFETCH_BUNDLE`.
  The `bundle-gen1` feature and the duplicate `sprites/` tree it read were
  removed; `assets/` is the single source of artwork. Runtime support for
  legacy configs that name a game in `sprites.variant` is unrelated and stays.
- Never push, tag, or publish a release without Michael's fresh confirmation.
- Later roadmap items are broader PokeAPI front-sprite coverage and a reusable
  rendering/service boundary; overworld directional animation is a separate
  future asset model, not part of the current greeting bundle.

Do not change native Claude, Codex, OpenCode, or dotfiles state as part of
Pokefetch feature work. Keep credentials, provider sessions, and machine-local
configuration outside this repository.
