# Configuration and command reference

Pokefetch works without a config file. By default it selects Red/Blue sprites
from Pokedex IDs 1–151, renders an eight-row centered image, and prepares the
next Ghostty dock icon when running in a local Ghostty session.

## Commands

```sh
pokefetch                              # the full greeting
pokefetch show pikachu                 # one Pokemon, no icon work
pokefetch palette eevee                # the eight extracted colors
pokefetch sprite 25                    # resolved sprite path
pokefetch icon 25 --output /tmp/P.icns # a macOS icon
pokefetch render 6 --output /tmp/x.png # a scaled PNG for inspection
pokefetch bundle                       # compiled bundle profile
```

Bare `pokefetch` is the same as `pokefetch greet`. Run `pokefetch --help` or
`pokefetch <command> --help` for the complete generated CLI reference.

Global options work on either side of a subcommand and override the config file
for one run without rewriting it:

```sh
pokefetch --game crystal --size 8 --alignment center
pokefetch --game gold --game silver --game crystal show celebi
pokefetch --game gold,crystal --size 2 --alignment top --no-icon
```

## Config file

Pokefetch reads `$XDG_CONFIG_HOME/pokefetch/config.toml`, normally
`~/.config/pokefetch/config.toml`. The repository's
[`config.example.toml`](../config.example.toml) contains the complete shape:

```toml
[sprites]
game = "random"
variant = "front"
artwork = false
range_start = 1
range_end = 386
pokemon = []

[display]
size = 8
alignment = "center"
gap = 2
background = "#222436"

[icon]
enabled = true
```

Every table and key is optional. Unspecified values retain their defaults.

### Sprite selection

`game` accepts one cataloged game, `"random"`, or a curated list. A fixed game
can download missing sprites on demand. `"random"` and curated lists select only
from the compiled bundle, and every game in a curated list must be bundled.

```toml
[sprites]
game = ["gold", "silver", "crystal"]
pokemon = [249, 250, 251]
```

`range_start` and `range_end` constrain random Pokedex selection. A non-empty
`pokemon` list of Pokedex IDs replaces that range as the random selection pool.

`variant = "front"` selects the normal front sprite. Crystal also exposes
`"front-animated"`; the current terminal renderer uses its first frame because
Ghostty does not yet implement Kitty animation frames.

`artwork = true` uses high-resolution official artwork instead of game sprites.
The `--artwork` and `--no-artwork` flags override it for one command.

### Display

Display size is measured in terminal rows, not pixels. `size = 8` produces an
eight-row image, a 16-column Kitty placement, and a 256-pixel render canvas. The
allowed range is 1–32 rows.

`alignment = "top"` starts the sprite and text on the same row.
`alignment = "center"` vertically centers whichever side is shorter. Pokefetch
does not add filler lines to make the two sides equal.

`gap` is the number of blank terminal columns between the sprite and text.
`background` is the terminal's `#RRGGBB` background color and is used to keep
palette colors legible.

The [README gallery](../README.md#more-examples) shows these settings in
different combinations.

### Dock icon

`icon.enabled = true` allows a local Ghostty greeting to prepare the icon for
the next Ghostty launch. `--no-icon` disables that work for one command, and
`--icon` forces it. See [Ghostty integration](ghostty.md) for the one-time icon
setup and the reason the icon trails the greeting by one launch.

## File locations

| Path | Contents |
|---|---|
| `$XDG_CONFIG_HOME/pokefetch/config.toml` | Settings |
| `$XDG_CONFIG_HOME/pokefetch/sprites/` | Local sprite overrides |
| `$XDG_CACHE_HOME/pokefetch/sprites/` | Downloaded sprites |
| `$XDG_CACHE_HOME/pokefetch/system.toml` | Cached machine facts |
| `$XDG_STATE_HOME/pokefetch/Ghostty.icns` | Generated dock icon |

Local overrides are resolved from `sprites/<game>/<variant>/<id>.<format>` and
win over bundled and downloaded assets.

Hardware and package summaries are cached because a greeting runs on every new
terminal. Re-running expensive system and package-manager probes during normal
shell startup would add visible latency.

## Terminal graphics fallback

Pokefetch prints five plain text lines when stdout is not a terminal or the
terminal does not advertise Kitty graphics. Redirected output is intentionally
free of image and color escape sequences.

Remote Ghostty sessions are recognized through `TERM=xterm-ghostty`, which SSH
carries even when it drops `TERM_PROGRAM`. `--force-kitty` covers compatible
terminals that do not identify themselves. See [Ghostty integration](ghostty.md)
for detection and SSH details.
