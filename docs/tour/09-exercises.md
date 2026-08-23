# 9. Exercises

Reading Rust and writing Rust teach different things. These are ordered roughly
by difficulty. Each one is a real change to a working program, and the compiler
will tell you when you are wrong.

Work on a branch:

```sh
git switch -c my-experiments
```

After each change:

```sh
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt
```

---

## Warm-up

### 1. Break a doc test

Open [`src/pokemon/mod.rs`](../../src/pokemon/mod.rs) and change the expected
value in the example on `Pokemon::label` from `#025 Pikachu` to `#026 Pikachu`.
Run `cargo test`.

The point: documentation in this project cannot silently drift.

### 2. Add a fallback color

[`src/palette.rs`](../../src/palette.rs) has a `FALLBACKS` table. Change one
color, then:

```sh
cargo run -- palette pikachu
```

Nothing changes. Why? Find out by reading `extract` — the fallbacks are only
used when a sprite yields no colors at all. Now try a fully transparent image
via the test `falls_back_when_every_pixel_is_transparent`.

The point: reading code to explain a non-result is most of debugging.

### 3. Make the array the wrong length

Change `FALLBACKS` to have seven entries. Read the error carefully.

The point: array length is part of the type in Rust. `[Color; 8]` and
`[Color; 7]` are different types, and this is a compile error rather than a
runtime surprise.

---

## Small features

### 4. Add a sixth information line

[`src/terminal/mod.rs`](../../src/terminal/mod.rs), `information_lines`. Add
something — uptime, the current directory, the time.

The layout already supports one through eight lines and the palette already has
eight colors, so this should require no other change. Verify that claim.

```sh
cargo run -- --size 8 show pikachu
```

The point: a well-factored codebase makes some changes tiny. Notice how you
found that out — by reading the doc comment on `greeting_layout`, which stated
the range.

### 5. Add a `--no-color` flag

Add it to [`src/cli.rs`](../../src/cli.rs), thread it through to
[`print_plain`](../../src/terminal/mod.rs), which already takes a `colored`
boolean.

The point: this is what a feature looks like end to end — CLI, config merge,
and the function that acts on it. Note that clap will generate the `--help`
entry from your doc comment automatically.

### 6. Add a config option

Give `[display]` a new key — say, `uppercase = true` — that uppercases the
information lines. You will need to touch `DisplayConfig`, its `Default` impl,
and validation if the value can be wrong.

The point: `#[serde(default)]` means old config files keep working. Confirm
that by running with no config file at all.

---

## Refactoring

### 7. Make the alignment exhaustive

Add a `Bottom` variant to `Alignment` in [`src/config.rs`](../../src/config.rs)
but do not implement it anywhere. Run `cargo build`.

The compiler lists every `match` you now have to handle. Implement it.

The point: this is the payoff of exhaustive matching. Adding a case to an enum
turns "hope you found every place" into a checklist the compiler writes.

### 8. Replace a `String` with a `&str`

`SpriteStore::variant()` returns a `String`, allocating on every call — and it
is called several times per run. Try making it return `&str`.

You will hit the borrow checker. Understanding *why* is the exercise: the
method builds its return value conditionally, so there is not always something
to borrow. Possible answers include `Cow<'_, str>`, computing it once in `new`
and storing it, or leaving it alone.

The point: not every allocation can be removed, and knowing which is which is
the skill.

### 9. Give the palette a real error type

Replace one `anyhow` usage with a hand-rolled enum implementing
`std::error::Error`, or use `thiserror`.

The point: feel the difference. `anyhow` is right for an application; a library
that wants callers to distinguish failures needs concrete types. Now you have
written both.

---

## Bigger

### 10. Add a new subcommand

Something like `pokefetch info <pokemon>` that prints the id, name, resolved
sprite path, and palette without drawing anything.

Everything you need is already public. This is mostly about finding out how
little new code a well-structured program needs.

### 11. Support a new sprite variant

`back` sprites exist upstream. Follow `front` through
[`src/sprite.rs`](../../src/sprite.rs) — `source_for_variant`,
`extension_for`, validation in [`src/config.rs`](../../src/config.rs) — and add
`back`.

Note the constraint in [`AGENTS.md`](../../AGENTS.md): back sprites are
deliberately shelved for the shipped bundles. Doing this on a branch to learn
is fine; the exercise is tracing a concept through a codebase.

### 12. Make the greeting configurable

Right now `information_lines` is hardcoded. Make the lines a config list, like
`lines = ["user", "system", "hardware", "pokemon"]`.

This touches config parsing, validation (unknown line names?), the layout
(which already handles one through eight), and probably a new enum. It is the
most realistic exercise here, because it starts as "just make it a list" and
turns into a design question.

---

## Reading, not writing

### 13. Find the shell-startup budget

[`AGENTS.md`](../../AGENTS.md) says interactive shell startup latency is a
primary constraint. Find every place the code pays for that:

- a cache with two different lifetimes
- a background process that is deliberately not waited on
- an expensive encode that is skipped when nothing will display it
- a palette computed at build time rather than at runtime

Then measure it:

```sh
time cargo run --release -- --no-icon greet
```

The point: performance work in a real codebase is a set of specific decisions
recorded in specific places, not a general property. Every one of those four has
a comment explaining itself — that is what "durable comments" means.

---

Previous: [The build script](08-build-script.md) · Back to [the tour](README.md)
