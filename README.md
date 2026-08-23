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
