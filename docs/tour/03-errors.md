# 3. Errors and absence

Rust has no exceptions and no `null`. Two enums cover both jobs:

```rust
enum Option<T> { Some(T), None }          // a value might not be there
enum Result<T, E> { Ok(T), Err(E) }       // an operation might have failed
```

Because they are ordinary types, the compiler makes you deal with them. You
cannot accidentally use a missing value.

## `Option`: absence

[`src/pokemon/mod.rs`](../../src/pokemon/mod.rs) takes `Option<&str>` for a
selector, because "the user typed nothing" is a real, meaningful state:

```rust
pub fn resolve(selector: Option<&str>, config: &SpriteConfig) -> Result<Pokemon>
```

The idiomatic way to peel off the `None` case is `let ... else`:

```rust
let Some(selector) = explicit_selector(selector) else {
    return Ok(from_id(random_id(config, &mut available)?));
};
```

The `else` block must diverge — return, break, or panic. That requirement is
what makes the rest of the function clean: after this line `selector` is a
plain `&str`, with no unwrapping anywhere.

Compare against what the code looked like before this pass:

```rust
if selector.is_none() || selector == Some("random") {
    return Ok(from_id(random_id(config, &mut available)?));
}
let selector = selector.expect("checked above");   // <- a comment doing a type's job
```

`expect("checked above")` works, but the compiler is not checking that claim.
`let ... else` moves the guarantee from a comment into the type system.

### `Option` combinators

You will see these constantly. All of them avoid an explicit `match`:

| Method | Meaning |
|--------|---------|
| `map(f)` | transform the value if present |
| `and_then(f)` | like `map`, but `f` returns another `Option` |
| `filter(p)` | keep `Some` only if the predicate holds |
| `unwrap_or(v)` | the value, or a default |
| `unwrap_or_else(f)` | the value, or a computed default |
| `map_or_else(d, f)` | transform if present, otherwise compute a default |
| `is_none_or(p)` | true if `None`, or if the predicate holds |

From [`src/terminal/mod.rs`](../../src/terminal/mod.rs):

```rust
let user = std::env::var("USER")
    .ok()                                   // Result -> Option
    .filter(|value| !value.is_empty())      // empty is as good as unset
    .map_or_else(|| "Trainer".to_string(), |value| capitalize(&value));
```

Read it top to bottom as a sentence: get `USER`, treat a failure as absent,
treat empty as absent, and either capitalize it or fall back to "Trainer".
The alternative is three nested `match` blocks that say the same thing.

## `Result` and `?`

`?` is the whole story: on `Ok`, unwrap and continue; on `Err`, return early.

```rust
let text = std::fs::read_to_string(&path)?;
```

It also works on `Option` in a function returning `Option`, which is why
`parse_hex` in [`src/palette.rs`](../../src/palette.rs) reads the way it does:

```rust
Some(Color::rgb(
    u8::from_str_radix(value.get(0..2)?, 16).ok()?,
    ...
))
```

## `anyhow`, and why this crate uses it

This is an **application**, not a library. Its errors are read by a person and
then the program exits. Nobody matches on them programmatically. So it uses
`anyhow::Result`, which is `Result<T, anyhow::Error>` — one error type that any
error converts into.

The payoff is `.context()`:

```rust
image_ops::load_rgba(&path).with_context(|| format!("loading {}", pokemon.label()))
```

Each layer adds a sentence, and the user sees the chain:

```text
pokefetch: loading #025 Pikachu: decoding /Users/you/.cache/.../25.png: invalid PNG header
```

That reads like an explanation instead of a stack trace.

Use `with_context` (a closure) when building the message costs something —
`format!` allocates — so it only runs on the error path. Use `context` for a
plain literal.

**If you were writing a library** other code depends on, you would define
concrete error types instead, usually with `thiserror`, so callers can match on
specific failures. Application versus library is the question that decides
this, not taste.

## Deciding not to handle an error

Not every failure deserves propagation. [`src/system.rs`](../../src/system.rs)
returns no `Result` at all:

```rust
if let Ok(text) = toml::to_string(&snapshot) {
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = std::fs::write(path, text);
}
```

`let _ =` explicitly discards a `Result`. This is not sloppiness — it is a
decision, and the syntax makes it visible. If the cache cannot be written, the
greeting should still print; it will just be slower next time. Failing a
shell's startup because a cache directory is read-only would be worse behavior,
not better.

The distinction: **propagate what the user can act on, absorb what they
cannot.** A malformed config file is worth an error; an unwritable cache is not.

## `unwrap`, `expect`, and panicking

`unwrap` and `expect` crash on failure. They are not forbidden, but each one
should be a claim you can defend:

```rust
.expect("clustering always has at least one center")
```

That message is not an apology, it is an invariant — the function returns early
when there are no centers, so reaching this line with an empty slice would be a
bug in this file, not bad input.

Good places: tests, and invariants the surrounding code guarantees. Bad places:
anything touching user input, files, or the network. Note that
[`tests/`](../../tests/) uses `unwrap` liberally, which is correct — a panic in
a test is just a failed test.

---

Previous: [Ownership](02-ownership.md) · Next: [Traits and generics](04-traits.md)
