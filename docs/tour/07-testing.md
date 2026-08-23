# 7. Testing

Rust ships its test framework in the language. No dependency, no config file.

```sh
cargo test                       # everything
cargo test palette               # tests whose name contains "palette"
cargo test --lib                 # unit tests only
cargo test --test cli            # one integration file
cargo test -- --nocapture        # let println! through
```

This crate has 78 tests in three kinds, and it uses all three on purpose.

## 1. Unit tests: inside the module

```rust
#[cfg(test)]
mod tests {
    use super::{greeting_layout, GreetingLayout};

    #[test]
    fn centers_shorter_text_or_image_by_terminal_row() { ... }
}
```

`#[cfg(test)]` means the module is compiled only under `cargo test` — it costs
nothing in a release build. `use super::*` (or a specific list) reaches into the
parent module.

The point of unit tests living inside the module is that **they can see private
items**. `greeting_layout`'s tests, `kitty_terminal_name`'s tests, and
`parse_hex`'s tests all exercise functions that are not `pub`. An outside test
could not reach any of them.

Convention: put them at the bottom of the file they test.

## 2. Integration tests: `tests/`

Each file in [`tests/`](../../tests/) is compiled as its own separate crate that
links against your library. That means it can only use `pub` items — exactly
what an outside user gets.

```rust
use pokefetch::config::Config;
use pokefetch::palette::{self, SIZE};
```

Note it says `pokefetch::`, not `crate::`. From here, this is just some
dependency.

That constraint is the value: if [`tests/greeting.rs`](../../tests/greeting.rs)
fails to compile, the public API is missing something. It is a check on your
API design, not only on your logic.

[`tests/cli.rs`](../../tests/cli.rs) goes one step further and runs the real
executable:

```rust
Command::new(env!("CARGO_BIN_EXE_pokefetch"))
```

Cargo builds the binary before integration tests and sets
`CARGO_BIN_EXE_<name>` to its path, so there is no path guessing. These tests
check things only reachable from outside: exit codes, `--help` output, and
whether an invalid flag is rejected before any work happens.

They also pin XDG paths at a nonexistent directory:

```rust
.env("XDG_CONFIG_HOME", "/nonexistent/pokefetch-test")
```

A test that reads your real config is not a test, it is a coin flip.

## 3. Doc tests: examples that must work

Any fenced code block in a `///` comment is compiled and run by `cargo test`:

```rust
/// ```
/// # use pokefetch::pokemon::resolve_by_name;
/// let pikachu = resolve_by_name("pikachu").unwrap();
/// assert_eq!(pikachu.label(), "#025 Pikachu");
/// ```
pub fn label(&self) -> String
```

**Documentation that lies fails the build.** Try it: change `#025` to `#026` and
run `cargo test`. This is the single best feature Rust has for keeping docs
honest, and it is why this crate's examples are worth trusting.

Lines starting with `# ` are hidden from the rendered docs but still compiled —
use them for imports and setup so the example shows only the interesting part.

For a block that should compile but not run, use ` ```no_run `. For one that is
not Rust at all (the ASCII diagrams in this crate), use ` ```text ` — otherwise
the test runner will try to compile your diagram.

## Naming tests

Compare:

```rust
#[test] fn test_layout() { ... }
#[test] fn centers_shorter_text_or_image_by_terminal_row() { ... }
```

The second names the behavior, so a failure reads as a statement about what
broke. Test names are the only documentation that is guaranteed to be accurate,
because a stale one shows up in the output.

## Assertions

```rust
assert!(condition);
assert_eq!(left, right);
assert!(result.is_err());
assert_eq!(cores.len(), 2, "system_profiler reports this key twice");
```

`assert_eq!` prints both values on failure, so prefer it over
`assert!(a == b)`. The trailing message argument is worth adding when the
assertion alone would not explain itself.

Note that `assert_eq!` needs `PartialEq` **and** `Debug` on the type — which is
why so many structs in this crate derive both.

## What is worth testing here

Look at where the tests actually cluster:

- **Pure functions** — `greeting_layout`, `parse_hex`, `color_distance_squared`,
  `capitalize`. Cheap to test, so test them thoroughly.
- **Parsing and validation** — every `Config` rejection has a test, because
  these are the errors real users hit.
- **Things with a stated invariant** — "extraction is deterministic",
  "nearest-neighbor preserves source colors", "no temporary file is left
  behind". If a doc comment claims something, a test should hold it to it.
- **Environment-dependent logic made testable** — `kitty_terminal_name` takes
  the environment as parameters precisely so its SSH behavior can be tested
  without an SSH session.

And where they do not: nothing tests `system_profiler` output parsing against a
real Mac, or downloads a sprite. Tests that need the network or specific
hardware are tests that fail for reasons unrelated to your code.

---

Previous: [Modules](06-modules.md) · Next: [The build script](08-build-script.md)
