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
  `cargo test --features bundle-gen1`, and
  `cargo clippy --all-targets --features bundle-gen1 -- -D warnings`.

## Handoff

This file is the provider-neutral project brief for Claude, Codex, OpenCode,
and other agents. Read `README.md`, `docs/assets-and-distribution.md`,
`release/README.md`, and the manifests before changing bundle or release
architecture.

Current direction:

- Keep `v0.1.0` unreleased until the hosted validation and personal Mac mini
  bootstrap have been exercised.
- Workflows are manual-only. Never push, tag, or publish a release without
  Michael's fresh confirmation.
- Next release work is to validate the current packaging workflows, then
  prepare the first release only after the Mac mini dry run is green.
- Later roadmap items are broader PokeAPI front-sprite coverage and a
  reusable rendering/service boundary; overworld directional animation is a
  separate future asset model, not part of the current greeting bundle.

Do not change native Claude, Codex, OpenCode, or dotfiles state as part of
Pokefetch feature work. Keep credentials, provider sessions, and machine-local
configuration outside this repository.
