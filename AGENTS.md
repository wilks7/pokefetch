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
