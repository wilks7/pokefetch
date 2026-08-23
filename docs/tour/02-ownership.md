# 2. Ownership and borrowing

This is the chapter that matters. Everything else in Rust is a normal language
feature; this is the part that is actually different.

## The rule

Every value has exactly one owner. When the owner goes out of scope, the value
is freed. You can hand out *references* to a value, and the compiler checks
that no reference outlives what it points at.

You can have either:

- any number of shared references (`&T`), or
- exactly one exclusive reference (`&mut T`)

but never both at once. That single rule is what eliminates data races and
use-after-free, without a garbage collector.

## Seeing it in this codebase

### A struct that borrows: `SpriteStore<'a>`

Open [`src/sprite.rs`](../../src/sprite.rs):

```rust
pub struct SpriteStore<'a> {
    config: &'a SpriteConfig,
    config_dir: &'a Path,
    game: String,
}
```

Two borrowed fields and one owned one. The `<'a>` is a **lifetime parameter**:
it says "this struct holds references, and it must not outlive them."

Why borrow the config instead of cloning it? Because a `SpriteConfig` contains
a `String` and a `Vec`, and cloning means allocating. The store only reads the
config, so borrowing is both cheaper and more honest about intent.

`game` is a `String` — owned — because the store *computes* it (a random game
choice, resolved once at construction). It is not pointing at something that
already exists, so there is nothing to borrow.

Try this: delete the `'a` from the struct definition and run `cargo build`. The
error tells you exactly what is missing and why.

### An exclusive borrow: `apply_overrides`

In [`src/cli.rs`](../../src/cli.rs):

```rust
pub fn apply_overrides(config: &mut Config, cli: &Cli)
```

`&mut Config` is exclusive. While `apply_overrides` runs, nothing else in the
program can read or write that config. In C++ you would rely on convention; in
Rust the compiler rejects the alternative.

Notice the asymmetry: `cli` is `&Cli` because the function only reads it.
Taking `&mut` for something you do not mutate compiles, but it is a lie about
what your function does, and readers will believe it.

### Owned vs borrowed strings

You will see three string-ish types constantly:

| Type | What it is | When |
|------|-----------|------|
| `String` | owned, heap-allocated, growable | you built it or need to keep it |
| `&str` | a borrowed view into text | you are only reading |
| `&'static str` | a view into text baked into the binary | literals, lookup tables |

[`src/pokemon/names.rs`](../../src/pokemon/names.rs) is `[&str; 386]`: 386
borrowed views into text that lives in the executable. Nothing is allocated to
load it, because there is nothing to load.

But `Pokemon::name` is a `String`. Why? Because `from_id` can produce a name
that does not exist in the table (`format!("Pokemon {id}")` for an unknown id).
That string is created at runtime, so someone has to own it.

This is the everyday version of the ownership question: **does this value
already live somewhere else, or am I making it?**

### `Copy`: the exception

In [`src/palette.rs`](../../src/palette.rs):

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Color { pub red: u8, pub green: u8, pub blue: u8 }
```

`Color` is three bytes. Copying it is cheaper than referencing it, so it is
`Copy`: assignment duplicates rather than moves, and you never think about
ownership for it again.

This is why `Color::hex` takes `self` and not `&self`:

```rust
pub fn hex(self) -> String
```

For a three-byte type, taking it by value is free.

The rule of thumb: small, fixed-size, no heap allocation → `Copy`. Anything
holding a `String`, a `Vec`, or a file handle → not `Copy`, because duplicating
it would mean duplicating what it owns.

## When borrowing gets awkward

Sometimes the borrow checker says no and the fix is to restructure, not to
fight it. In [`src/palette.rs`](../../src/palette.rs), `order_for_display`
starts with:

```rust
let colors = centers.clone();
```

A clone! Isn't that the thing we were avoiding? Yes — and here it is correct.
The function needs to read the full set of centers *while* mutating a
collection derived from them. Cloning a small `Vec<Color>` costs a few dozen
bytes and makes the code obviously correct. The alternative (indices, or
splitting the loop) would be faster in a way nobody could measure and harder to
read.

**Clone when you are stuck and the data is small.** The mistake is cloning
inside a hot loop over large data without noticing, not cloning at all.

## What to take away

- Borrow to read, `&mut` to write, own when you made it.
- Lifetimes are not something you write everywhere; they appear when a struct
  holds a reference, and the compiler usually infers the rest.
- `Copy` is for small plain data.
- A `.clone()` you thought about is fine. A `.clone()` you added to silence an
  error you did not read is a smell.

---

Previous: [Orientation](01-orientation.md) · Next: [Errors and absence](03-errors.md)
