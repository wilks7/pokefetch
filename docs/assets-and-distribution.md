# Assets and distribution

Pokefetch deliberately separates canonical artwork, build-time bundles, and
published binaries. This keeps today's terminal greeting fast without making
its storage model the permanent architecture for every future sprite use.

## Current ownership

- `manifests/sets.toml` pins upstream repositories and describes game-oriented
  sprite sets.
- `assets/manifest.toml` records every imported file, digest, format, and
  derived terminal palette.
- `assets/sets` stores the curated upstream bytes required for reproducible and
  offline builds.
- `manifests/bundles.toml` selects source assets for a compiled executable.

Which profile a given build embeds is chosen at build time by
`POKEFETCH_BUNDLE`, not by a separate release manifest. The manual release
workflow builds one flavor (`retro-master`); anything else is a local build.

The complete source corpus is currently small enough for ordinary Git. Revisit
that choice based on measured clone and repository cost, not merely asset
count. Git LFS or separately versioned asset packs become candidates only when
normal Git materially harms development or distribution.

`retro-master` means every supported asset selected by that profile at a given
Git tag. As later PokeAPI games are imported, a tagged release continues to pin
the exact historical contents while newer releases may expand the profile.
Compact profiles remain useful for constrained installations and for keeping
local build times down, but a release does not need to prebuild every possible
combination — one complete flavor plus buildable sources covers both audiences.

## Building a bundle

A normal build embeds no sprites. It downloads missing artwork on demand and
caches it locally:

```sh
cargo build --release --locked
```

Enable `bundle-assets` and set `POKEFETCH_BUNDLE` to compile an offline binary:

```sh
POKEFETCH_BUNDLE=retro-master \
  cargo build --release --locked --features bundle-assets
```

The same selection works with `cargo install`:

```sh
POKEFETCH_BUNDLE=crystal-full \
  cargo install --path . --root ~/.local --locked --features bundle-assets
```

Each game set has two profiles:

- `<game>-core` includes the set's explicit core Pokedex ranges.
- `<game>-full` includes every imported species that game set provides.

The available game prefixes are `red-blue`, `yellow`, `gold`, `silver`,
`crystal`, `ruby-sapphire`, `emerald`, and `firered-leafgreen`.
`retro-master` combines every full set through FireRed/LeafGreen. The canonical
definitions live in [`manifests/bundles.toml`](../manifests/bundles.toml).

Inspect a built or installed executable with:

```sh
pokefetch bundle
```

Bundle contents and runtime sprite selection are separate. A binary may contain
many game sets while a config selects one game, a curated pool, or `random`.

## Sprite catalog and palettes

Pokefetch models artwork by game, not by the generation in which a Pokemon
debuted. FireRed/LeafGreen, for example, is a Generation III game set whose core
range is Kanto.

Active greeting artwork is always front-facing and background-transparent.
Generations I and II use PokeAPI's transparent renderings; Generation III front
sprites already carry alpha. Back, shiny, gray, and opaque source variants are
intentionally not active.

Every imported asset carries an eight-color, population-weighted palette
computed during the asset pipeline. The extractor balances dominant coverage
against color separation, preserves opaque white, and repeats real sprite
colors when older artwork has fewer than eight. The greeting currently consumes
five palette entries.

List the set and bundle catalog with:

```sh
cargo run --bin pokefetch-assets -- list
```

## Importing assets

Imports require a local checkout of the upstream PokeAPI sprites repository at
the exact revision pinned in [`manifests/sets.toml`](../manifests/sets.toml).
Imports are dry runs unless `--apply` is given.

```sh
git clone --filter=blob:none --no-checkout \
  https://github.com/PokeAPI/sprites.git /tmp/pokeapi-sprites
git -C /tmp/pokeapi-sprites sparse-checkout set \
  sprites/pokemon/versions/generation-i \
  sprites/pokemon/versions/generation-ii \
  sprites/pokemon/versions/generation-iii
git -C /tmp/pokeapi-sprites checkout c10459b9b0129eaca5c5d9b1cac65336debb1d08

cargo run --bin pokefetch-assets -- import \
  --source /tmp/pokeapi-sprites \
  --set crystal

# Review the counts, then apply the import.
cargo run --bin pokefetch-assets -- import \
  --source /tmp/pokeapi-sprites \
  --set crystal \
  --apply
```

Applied imports preserve upstream bytes, validate each image, record its SHA-256
digest and eight-color palette, prune stale variants, and atomically update
`assets/manifest.toml`.

Crystal also exposes `front-animated`. Its transparent GIFs are bundled and
decoded, but the terminal renderer uses the first frame because Ghostty does not
yet implement Kitty animation frames.

Configs that put a legacy game name in `sprites.variant`, such as
`variant = "red-blue"`, remain supported by a runtime compatibility shim. That
compatibility behavior is separate from current asset storage and bundles.

## Future rendering and serving boundary

Terminal layout, asset lookup, decoding, nearest-neighbor scaling, and output
transport should remain separable. If Pokefetch grows a local service or is
split into workspace crates, the reusable core should accept an asset identity
and render request and return pixels or encoded bytes without depending on
Fish, Ghostty, Kitty placement, HTTP, or a long-running process.

A future service may expose cached sprites at requested integer sizes, while
the current CLI can continue embedding a selected offline bundle. Release
assets and runtime asset packs are separate concepts: an executable archive is
not the canonical sprite database.

## Overworld animation

Directional overworld sprites are not battle `front` variants. Importing them
will require a separately pinned and licensed source plus metadata for at least:

- subject or form identity;
- direction (`up`, `down`, `left`, or `right`);
- animation sequence and frame order;
- source dimensions and integer scaling constraints;
- transparency, digest, and upstream provenance.

The renderer can later animate those frames in a TUI or serve individual
frames, but the current manifest should not encode direction in filenames or
pretend those assets belong to a game-front bundle. Design that schema when a
specific upstream corpus and consumer are selected.
