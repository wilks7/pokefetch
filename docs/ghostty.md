# Ghostty integration

Pokefetch targets Ghostty for two separate features. They are independent — you
can use either without the other.

1. **Inline sprites**, via the Kitty graphics protocol.
2. **A rotating dock icon**, via a generated `.icns`.

## 1. Inline sprites

Ghostty implements the Kitty graphics protocol, so no configuration is needed.
Pokefetch detects support and falls back to plain colored text otherwise.

Detection reads three environment variables, in
[`src/terminal/detect.rs`](../src/terminal/detect.rs):

| Variable | Set by | Survives SSH |
|----------|--------|--------------|
| `TERM_PROGRAM=ghostty` | Ghostty | no |
| `TERM=xterm-ghostty` | the terminfo entry | yes |
| `KITTY_WINDOW_ID` | Kitty only | no |

`TERM` is the one that matters remotely, because SSH carries it with the
allocated pseudo-terminal while `TERM_PROGRAM` is dropped.

For `TERM=xterm-ghostty` to be meaningful on the far side, that terminfo entry
has to exist there. Ghostty can install it for you:

```
shell-integration-features = ssh-env,ssh-terminfo
```

`ssh-terminfo` copies the entry to hosts you connect to; `ssh-env` sets the
variables. Without them a remote shell falls back to `xterm-256color` and
Pokefetch correctly prints text instead of a broken image.

For a terminal that supports the protocol but does not identify itself:

```sh
pokefetch greet --force-kitty
```

### Sizing

Pokefetch sizes images in **rows and columns**, not pixels:

```text
size = 8  ->  c=16, r=8   (16 columns x 8 rows)  ->  256px render canvas
```

That is why a sprite stays proportional when you change Ghostty's font size —
the placement is in cells, so it scales with the grid. Rendering at a fixed
pixel size would break every time the font changed.

### Animation

Crystal ships animated GIFs and Pokefetch bundles them, but only the first
frame is rendered. Ghostty does not yet implement Kitty animation frames
([ghostty#5255](https://github.com/ghostty-org/ghostty/issues/5255)). A
repaint loop on the shell-startup path is deliberately not an option — see the
latency constraint in [`AGENTS.md`](../AGENTS.md).

## 2. The rotating dock icon

Ghostty reads its icon **at launch**, so Pokefetch writes the icon for the
*next* window during the current greeting. The icon therefore always trails the
greeting by one launch. That is the design, not a bug.

### Setup

Ghostty's `macos-icon = custom` makes it read `Ghostty.icns` from its own
config directory. Point that at the file Pokefetch generates:

```sh
# ~/.config/ghostty/config
macos-icon = custom
```

```sh
mkdir -p ~/.local/state/pokefetch
ln -s ~/.local/state/pokefetch/Ghostty.icns ~/.config/ghostty/Ghostty.icns
```

A symlink keeps generated data out of your dotfiles repository while letting
Ghostty find it at a stable path. Seed it once so the link is not dangling:

```sh
pokefetch icon pikachu
```

Validate:

```sh
ghostty +validate-config
```

### When it runs

Icon generation is skipped unless all of these hold:

- `icon.enabled` is true (default; `--no-icon` overrides for one run)
- `TERM_PROGRAM` is `ghostty`
- neither `SSH_CONNECTION` nor `SSH_TTY` is set

The SSH check matters: over SSH the icon would be written on the *remote*
machine, where no Ghostty will ever read it.

The work happens in a detached background process with all three standard
streams closed, so it cannot block your prompt or print over it after the fact.
See `schedule_icon` in [`src/app.rs`](../src/app.rs).

The write itself is atomic — a temporary file renamed into place — so Ghostty
launching mid-write reads either the old icon or the new one, never a truncated
file.

### Generating one by hand

```sh
pokefetch icon 25 --output /tmp/Pikachu.icns
pokefetch icon eevee                 # writes to the default state path
```

## Troubleshooting

**Text instead of a sprite.** Check what Pokefetch sees:

```sh
echo $TERM $TERM_PROGRAM
```

Piping also disables images by design — `pokefetch greet | cat` gives clean
text rather than escape codes.

**The icon never changes.** Ghostty reads it at launch, so quit and reopen
rather than opening a new window. Confirm the file is being written:

```sh
ls -l ~/.local/state/pokefetch/Ghostty.icns
readlink ~/.config/ghostty/Ghostty.icns
```

**The icon is one Pokemon behind.** Expected — see above.

**Nothing appears in a new shell.** The greeting is wired up separately, per
shell. See [`shell/README.md`](../shell/README.md).
