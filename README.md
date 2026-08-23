# Pokefetch

A terminal greeting that draws a Pokemon sprite beside your machine details,
colored from that sprite's own palette. Written in Rust.

```text
        ▄▄▄            Trainer @ studio
     ▄█▀   ▀█▄         macOS 15.3 · Apple M2
    █▀  ▄ ▄  ▀█        8C CPU · 10C GPU · 16GB RAM
    █   ▀▄▀   █        Fish · Ghostty · 214 Brew
     ▀█▄▄▄▄▄█▀         #025 Pikachu · red-blue/front
```

It also generates a matching macOS dock icon for Ghostty, so your terminal icon
changes with the greeting.

> **New to Rust?** This repository doubles as a worked example. See
> [`docs/tour/`](docs/tour/README.md) for a guided walkthrough of the codebase,
> and `cargo doc --open` for API documentation generated from the source.

## Install

### From source

```sh
git clone https://github.com/wilks7/pokefetch
cd pokefetch
cargo install --path . --root ~/.local --locked
```

That gives you a working binary that downloads sprites on demand and caches
them. For a fully offline binary with artwork baked in, pick a bundle:

```sh
POKEFETCH_BUNDLE=retro-master \
  cargo install --path . --root ~/.local --locked --features bundle-assets
```

`retro-master` is every imported game through FireRed/LeafGreen (~6 MB).
`red-blue-core` is the compact floor (~3.5 MB). Check what a binary contains:

```sh
pokefetch bundle
```

### From a release

Download the archive from the
[releases page](https://github.com/wilks7/pokefetch/releases), unpack it, and
put `pokefetch` on your `PATH`. Release binaries are built with
`retro-master`, so they need no network at all.

## Use it

```sh
pokefetch                              # the full greeting
pokefetch show pikachu                 # one Pokemon, no icon work
pokefetch palette eevee                # the eight extracted colors
pokefetch sprite 25                    # resolved sprite path
pokefetch icon 25 --output /tmp/P.icns # a macOS icon
pokefetch render 6 --output /tmp/x.png # a scaled PNG, for inspection
pokefetch bundle                       # what artwork is compiled in
```

Bare `pokefetch` is the same as `pokefetch greet`. Run `pokefetch --help` for
the full option list.

Global options work on either side of a subcommand and override the config file
for that one run, without rewriting it:

```sh
pokefetch --game crystal --size 8 --alignment center
pokefetch --game gold --game silver --game crystal show celebi
pokefetch --game gold,crystal --size 2 --alignment top --no-icon
```

## Shell integration

To greet yourself on every new terminal, see [`shell/README.md`](shell/README.md).
For Fish:

```fish
mkdir -p ~/.config/fish/functions
ln -s (pwd)/shell/fish_greeting.fish ~/.config/fish/functions/fish_greeting.fish
```

For the Ghostty dock icon and the Kitty graphics details, see
[`docs/ghostty.md`](docs/ghostty.md).

## Configure it

Configuration is optional — without a file Pokefetch uses Red/Blue, IDs 1–151,
size 8, centered. To change that, write
`$XDG_CONFIG_HOME/pokefetch/config.toml` (normally
`~/.config/pokefetch/config.toml`). See
[`config.example.toml`](config.example.toml) for the full shape.

```toml
[sprites]
game = "random"          # a name, "random", or a list
variant = "front"
range_start = 1
range_end = 386

[display]
size = 8                 # sprite height in terminal rows, 1-32
alignment = "center"     # or "top"
gap = 2
background = "#222436"   # your terminal background, for contrast correction

[icon]
enabled = true
```

`game` also accepts a curated pool. Every listed game must be present in the
compiled bundle:

```toml
[sprites]
game = ["gold", "silver", "crystal"]
```

### Sizing

Display size is expressed in **terminal rows**, not pixels. `size = 8` produces
an eight-row image, a 16-column Kitty placement, and a 256-pixel render canvas,
so sprites scale consistently with your terminal's font size. The range is 1
through 32 rows.

`alignment = "center"` vertically centers whichever side is shorter — the image
or the text. Pokefetch never adds filler lines to pad one out.

### Where files go

| Path | Contents |
|------|----------|
| `$XDG_CONFIG_HOME/pokefetch/config.toml` | your settings |
| `$XDG_CONFIG_HOME/pokefetch/sprites/` | local sprite overrides |
| `$XDG_CACHE_HOME/pokefetch/sprites/` | downloaded sprites |
| `$XDG_CACHE_HOME/pokefetch/system.toml` | cached machine facts |
| `$XDG_STATE_HOME/pokefetch/Ghostty.icns` | the generated icon |

Local overrides are resolved from `sprites/<game>/<variant>/<id>.<format>` and
win over everything else.

Hardware and package summaries are cached because a greeting runs on every new
terminal, and re-probing would make shell startup noticeably slow.

### Graphics fallback

The greeting prints five plain text lines when stdout is not a terminal or the
terminal does not advertise Kitty graphics. Remote Ghostty sessions are
recognized through `TERM=xterm-ghostty`, which SSH carries even though it drops
`TERM_PROGRAM`. `--force-kitty` covers compatible terminals that do not
identify themselves.

## Sprite sets and bundles

Pokefetch models artwork **by game**, not by the generation a Pokemon debuted
in. That is why a FireRed/LeafGreen core is correctly Kanto even though the
game is Generation III.

- `manifests/sets.toml` pins the upstream PokeAPI revision and describes each
  game set and its active variants.
- `manifests/bundles.toml` defines build-time content profiles. Each game has a
  compact `-core` profile and a `-full` profile; `retro-master` combines
  everything through FireRed/LeafGreen.

Active artwork is always background-transparent: Generations I and II use
PokeAPI's transparent renderings, and Generation III front sprites already carry
alpha. Back, shiny, gray, and opaque originals are intentionally shelved.

Every asset carries an eight-color, population-weighted palette computed at
build time. The extractor balances dominant coverage against color separation,
preserves opaque white, and repeats real sprite colors when older artwork has
fewer than eight. The renderer accepts one through eight lines and currently
uses five.

List the catalog:

```sh
cargo run --bin pokefetch-assets -- list
```

### Importing assets

Imports need a local checkout at the exact pinned revision, and are dry runs
unless `--apply` is given:

```sh
git clone --filter=blob:none --no-checkout \
  https://github.com/PokeAPI/sprites.git /tmp/pokeapi-sprites
git -C /tmp/pokeapi-sprites sparse-checkout set \
  sprites/pokemon/versions/generation-i \
  sprites/pokemon/versions/generation-ii \
  sprites/pokemon/versions/generation-iii
git -C /tmp/pokeapi-sprites checkout c10459b9b0129eaca5c5d9b1cac65336debb1d08

cargo run --bin pokefetch-assets -- import --source /tmp/pokeapi-sprites --set crystal
# review the counts, then:
cargo run --bin pokefetch-assets -- import --source /tmp/pokeapi-sprites --set crystal --apply
```

Applied imports preserve the original bytes, validate each image, record its
SHA-256 digest and eight-color palette, prune stale variants, and atomically
update `assets/manifest.toml`.

Crystal also exposes `variant = "front-animated"`. The transparent GIFs are
bundled and decoded, but only the first frame is rendered — Ghostty does not yet
implement Kitty animation frames.

## Develop

```sh
cargo test                                          # 78 tests, all offline
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo doc --open
```

Before committing Rust changes, also run the bundle-feature variants:

```sh
cargo test --features bundle-gen1
cargo clippy --all-targets --features bundle-gen1 -- -D warnings
```

Lints are configured in `Cargo.toml` under `[lints]`, at `clippy::pedantic`.

## Credits

Sprites come from the [PokeAPI sprites repository](https://github.com/PokeAPI/sprites)
at a pinned revision. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

Pokemon is a trademark of Nintendo, Creatures Inc., and GAME FREAK Inc. This is
an unaffiliated personal project.

## License

MIT — see [`LICENSE`](LICENSE).
