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

For an offline build with the tracked first-generation sprites and their
precomputed terminal palettes embedded in the binary, opt into the feature
when installing:

```sh
cargo install --path ~/Developer/pokefetch --root ~/.local --force --locked --features bundle-gen1
```

The default greeting chooses one of the original 151 Pokemon, displays its
`red-blue` sprite through the Kitty graphics protocol, colors five compact
machine/Pokemon lines from the sprite's palette, and atomically prepares
`~/.local/state/pokefetch/Ghostty.icns` for the next Ghostty launch. Icon work
is skipped automatically outside a local Ghostty session; the explicit `icon`
command remains available everywhere.

Personal configuration lives at `$XDG_CONFIG_HOME/pokefetch/config.toml`
(normally `~/.config/pokefetch/config.toml`); `config.example.toml` documents
the supported shape. Project sprites are resolved from
`sprites/<variant>/<id>.png`; missing sprites fall back to PokeAPI's public
sprite repository and are cached by variant under
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
describes each game set and its available PNG variants. The explicit core
roster is separate from the set's full upstream coverage; this lets a
FireRed/LeafGreen core correctly remain Kanto even though the game belongs to
Generation III.

`manifests/bundles.toml` defines build-time content profiles. Each game has a
compact `-core` profile and a `-full` profile containing every species that
the upstream game set provides. `retro-master` combines every cataloged game,
species, and PNG variant through FireRed/LeafGreen. Bundle contents and
runtime selection policy are intentionally independent, so a future client
can choose games, front/back artwork, and shiny odds from whatever its binary
contains.

The current `bundle-gen1` Cargo feature remains the working Red/Blue
transparent-sprite bundle while the new profiles are connected to the runtime.
The manifests and importer establish that next pipeline without changing the
interactive greeting yet.

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

Applied imports preserve the original bytes under `assets/sets`, validate
each PNG, calculate its SHA-256 digest and terminal palette, and atomically
update `assets/manifest.toml`. Crystal's animated GIFs are not part of the
PNG-first catalog yet.
