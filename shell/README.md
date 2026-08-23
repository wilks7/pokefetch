# Shell integration

## Fish

Fish's greeting is a function, not a variable or a startup script. Autoload it
by linking this file under its own name:

```fish
mkdir -p ~/.config/fish/functions
ln -s (pwd)/shell/fish_greeting.fish ~/.config/fish/functions/fish_greeting.fish
```

Open a new terminal to see it. To check it without one:

```fish
fish_greeting
```

To turn it off temporarily, without unlinking:

```fish
set --universal --export POKEFETCH_NO_GREETING 1
```

To turn it back on:

```fish
set --erase POKEFETCH_NO_GREETING
```

### Skipping nested shells

Running `fish` inside a Fish shell greets you again. If that bothers you, add a
depth check to the function:

```fish
test "$SHLVL" -gt 1; and return
```

This is left out by default because the starting value of `SHLVL` depends on how
the terminal launches your shell, and a wrong threshold means the greeting
silently never appears. Check yours with `echo $SHLVL` in a fresh window before
picking a number.

### Why a function rather than `config.fish`

Fish reads `~/.config/fish/config.fish` for *every* shell, scripts included, so
printing a greeting there would corrupt the output of anything that pipes a
Fish script. `fish_greeting` is called only for interactive shells, which is
exactly the condition Pokefetch wants.

## Bash and Zsh

Neither has a greeting hook, so guard on interactivity yourself. In
`~/.bashrc` or `~/.zshrc`:

```sh
case $- in
  *i*) command -v pokefetch >/dev/null && pokefetch greet ;;
esac
```

The `case` tests for `i` in the shell's option flags. `.bashrc` is not read by
non-interactive shells on most systems, but the check costs nothing and makes
the intent explicit.
