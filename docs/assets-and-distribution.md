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
`POKEFETCH_BUNDLE`, not by a separate release manifest. The release workflow
builds one flavor (`retro-master`); anything else is a local build.

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
