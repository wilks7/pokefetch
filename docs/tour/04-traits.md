# 4. Traits and generics

A trait is a set of behaviors a type can implement — close to an interface, but
you can implement a trait for a type you did not define.

## `derive`: traits for free

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pokemon { pub id: u16, pub name: String }
```

`derive` is a macro that writes the implementation for you, field by field.
`Eq`/`PartialEq` compares every field; `Clone` clones every field; `Debug`
formats every field.

**Derive only what you use.** Each one generates code. In this crate:

- `Debug` — needed for `{:?}` and for assertion failure messages in tests
- `Clone` — a caller needs an independent copy
- `Copy` — the type is small enough that copying beats referencing
- `Eq`, `PartialEq` — `assert_eq!` and `==`
- `Ord`, `PartialOrd` — sorting, and `BTreeSet`/`BTreeMap` keys
- `Default` — a meaningful zero value
- `Deserialize` / `Serialize` — serde, covered below

### Derive has consequences

Deriving `Ord` on [`Color`](../../src/palette.rs) orders lexicographically by
**field declaration order**: red, then green, then blue. That means reordering
the struct's fields silently changes how colors sort. The doc comment on
`Color` says so, because that is exactly the kind of thing a reader would
otherwise have to discover by breaking it.

## `Default`: what "empty" means

Derived `Default` gives you zeros and empty strings. Sometimes that is right;
in [`src/config.rs`](../../src/config.rs) it is not:

```rust
impl Default for DisplayConfig {
    fn default() -> Self {
        Self { size: 8, alignment: Alignment::Center, gap: 2,
               background: "#222436".to_string() }
    }
}
```

A derived default would be `size: 0`, which validation rejects. The hand-written
one is *runnable*, and that is the whole reason Pokefetch works with no config
file at all.

This pairs with a serde feature. `#[serde(default)]` on a struct means every
missing field falls back to that type's `Default`, which is what makes an
empty file, a partial file, and no file all behave identically. That property
is tested in `a_partial_table_keeps_the_other_defaults`.

Note the `..` shorthand for building from a default:

```rust
let display = DisplayConfig { size: 2, ..DisplayConfig::default() };
```

Set what you care about, inherit the rest. You will see this all over the tests.

## `From`: conversion

```rust
impl From<AlignmentArg> for Alignment {
    fn from(alignment: AlignmentArg) -> Self { ... }
}
```

Implementing `From` gives you `.into()` for free, in both directions of
inference. That is why [`src/cli.rs`](../../src/cli.rs) can write:

```rust
config.display.alignment = alignment.into();
```

The target type comes from the assignment, so `into()` knows what to produce.

You will also see this pay off in `GameSelection`, which implements
`From<&str>`. That is what makes `"red-blue".into()` work inside
`SpriteConfig::default()`.

Implement `From`, not `Into` — the standard library gives you `Into`
automatically from a `From` impl, and never the other way around.

## Traits as parameters

Three ways to say "any type that does X":

```rust
fn transmit(writer: &mut impl Write, ...)                 // impl Trait
fn resolve_available(..., available: impl FnMut(u16) -> bool)
fn nearest_center(color: Color, centers: &[Color]) -> usize   // no generics needed
```

`impl Write` in [`src/terminal/kitty.rs`](../../src/terminal/kitty.rs) is what
makes the Kitty protocol testable. In production the writer is a locked stdout;
in the tests it is a `Vec<u8>`, so the test can assert on the exact escape bytes:

```rust
let mut output = Vec::new();
transmit(&mut output, b"tiny", 16, 8).unwrap();
assert!(escaped.starts_with("\x1b_Ga=T,f=100,q=2,C=1,c=16,r=8,m=0;"));
```

Had `transmit` taken a concrete `Stdout`, none of that would be reachable. This
is the single highest-value habit in the chapter: **accept the most general
type you can actually use.**

The closure version does the same job for behavior. `resolve_available` takes
`impl FnMut(u16) -> bool` so the caller decides what "available" means, and the
`pokemon` module never learns that sprite bundles exist.

The three closure traits, in order of what they may do:

| Trait | Can it... |
|-------|-----------|
| `Fn` | be called repeatedly, capturing by reference |
| `FnMut` | mutate what it captured |
| `FnOnce` | consume what it captured, so callable once |

`FnMut` here because the caller's closure may want to cache lookups.

## Returning `impl Trait`

[`src/system.rs`](../../src/system.rs):

```rust
fn hardware_values<'a>(output: &'a str, key: &'a str) -> impl Iterator<Item = String> + 'a
```

"Returns some iterator; the concrete type is my business." The real type is an
unnameable closure-carrying `FilterMap`, so this is the only reasonable way to
write it.

The `+ 'a` matters: the returned iterator borrows `output`, so it must not
outlive it. And `move` in the closure body transfers the references in, rather
than borrowing locals that vanish when the function returns.

## Traits you get from dependencies

`#[derive(Parser)]` (clap) and `#[derive(Deserialize)]` (serde) are the same
mechanism, just supplied by a library. clap reads your struct and generates an
argument parser; serde reads it and generates a deserializer.

This is why [`src/cli.rs`](../../src/cli.rs) has a doc comment on every field:
clap turns each `///` into `--help` text. Documenting the field and implementing
the help output are the same act.

---

Previous: [Errors](03-errors.md) · Next: [Iterators](05-iterators.md)
