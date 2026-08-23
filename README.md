# Pokefetch

Pokefetch is the Rust-owned interactive-shell greeting and Ghostty icon
generator. It keeps the selected sprite variants in this repository while
placing downloaded fallbacks and generated state under XDG cache/state paths.

```sh
cargo install --path ~/Developer/pokefetch --root ~/.local --force --locked
pokefetch greet
pokefetch show pikachu
pokefetch palette eevee
pokefetch icon 25 --output /tmp/Pikachu.icns
```

For an offline build, choose a named profile and embed its imported sprites
and precomputed terminal palettes:

```sh
POKEFETCH_BUNDLE=red-blue-core \
  cargo install --path ~/Developer/pokefetch --root ~/.local --force --locked \
  --features bundle-assets

pokefetch bundle
```

Use `POKEFETCH_BUNDLE=retro-master` for every imported game through
FireRed/LeafGreen. The current corpus produces an approximately 6 MB release
binary for `retro-master`, compared with 3.5 MB for `red-blue-core`.

The default greeting chooses one of the original 151 Pokemon, displays its
`red-blue` sprite through the Kitty graphics protocol, colors five compact
machine/Pokemon lines from the sprite's palette, and atomically prepares
`~/.local/state/pokefetch/Ghostty.icns` for the next Ghostty launch. Icon work
is skipped automatically outside a local Ghostty session; the explicit `icon`
command remains available everywhere.

Personal configuration lives at `$XDG_CONFIG_HOME/pokefetch/config.toml`
(normally `~/.config/pokefetch/config.toml`); `config.example.toml` documents
the supported shape. Local overrides are resolved from
`sprites/<game>/<variant>/<id>.<format>`; missing bundled or local sprites fall
back to the pinned PokeAPI revision and are cached by game and variant under
`$XDG_CACHE_HOME/pokefetch/sprites`.

Hardware and package-manager summaries are cached in
`$XDG_CACHE_HOME/pokefetch/system.toml` so opening a new terminal does not
re-run system probes on every greeting.

Running `pokefetch` with no subcommand is equivalent to `pokefetch greet`.
The greeting falls back to five plain text lines when stdout is not a terminal
or the terminal does not advertise Kitty graphics support.

The tracked sprites originate from the
[PokeAPI sprites repository](https://github.com/PokeAPI/sprites).

## Sprite sets and bundles

Pokefetch models artwork by game rather than by the generation in which a
Pokemon debuted. `manifests/sets.toml` pins the upstream PokeAPI revision and
describes each game set and its active front-facing variants. The explicit core
roster is separate from the set's full upstream coverage; this lets a
FireRed/LeafGreen core correctly remain Kanto even though the game belongs to
Generation III.

`manifests/bundles.toml` defines build-time content profiles. Each game has a
compact `-core` profile and a `-full` profile containing every species that
the upstream game set provides. `retro-master` combines every imported game,
species, and active variant through FireRed/LeafGreen. Active static artwork
is always background-transparent: Gen I and II use PokeAPI's transparent
renderings, while Gen III's normal front sprites already contain alpha. Back,
shiny, gray, and opaque originals are intentionally shelved.

`bundle-assets` reads `POKEFETCH_BUNDLE` at build time and generates a compact,
sorted runtime index. Configuration selects the game and variant independently.
Use `game = "random"` to choose among the games present in the compiled bundle;
Pokefetch then limits random Pokemon selection to sprites present in that game:

```toml
[sprites]
game = "random"
variant = "front"
range_start = 1
range_end = 386
```

The legacy `variant = "red-blue"` configuration and `bundle-gen1` Cargo
feature remain compatible during migration.

Every asset carries an eight-color, population-weighted palette. The extractor
balances dominant coverage with color separation, preserves opaque white, and
repeats real sprite colors when older artwork exposes fewer than eight. The
greeting renderer accepts one through eight information lines and currently
uses five, leaving three palette slots available for future rows.

List the catalog and bundle profiles with:

```sh
cargo run --bin pokefetch-assets -- list
```

Asset imports require a local checkout at the exact pinned revision. Imports
are dry runs unless `--apply` is given:

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

# After reviewing the counts:
cargo run --bin pokefetch-assets -- import \
  --source /tmp/pokeapi-sprites \
  --set crystal \
  --apply
```

Applied imports synchronize the selected game directories under `assets/sets`,
preserve the original bytes, validate each image, calculate its SHA-256 digest
and eight-color first-frame terminal palette, prune stale managed variants,
and atomically update `assets/manifest.toml`.

Crystal also exposes `variant = "front-animated"`. Its original transparent
GIFs are bundled and decoded, but Pokefetch currently renders the first frame
as a static PNG because Ghostty does not yet implement Kitty graphics animation
frames. Native playback can be added without reacquiring assets when
[Ghostty animation support](https://github.com/ghostty-org/ghostty/issues/5255)
lands; a background repaint loop is deliberately avoided on shell startup.
