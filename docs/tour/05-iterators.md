# 5. Iterators

Rust code is full of long method chains. They look dense until you learn to
read them as a pipeline, top to bottom.

Iterators are **lazy**: nothing happens until something consumes them. A chain
of `.map().filter()` builds a description of work. `.collect()`, `.count()`,
`.sum()`, or a `for` loop is what actually runs it.

## Reading a chain

From [`src/sprite.rs`](../../src/sprite.rs):

```rust
let candidates = bundled::GAMES
    .iter()                                          // walk every bundled game
    .copied()                                        // &&str -> &str
    .filter(|game| requested.is_none_or(...))        // keep requested ones
    .filter(|game| species.map_or_else(...))         // keep ones with this sprite
    .collect::<Vec<_>>();                            // run it, gather results
```

Read one line at a time and ask what is flowing through. The `<Vec<_>>` on
`collect` tells the compiler what to build; the `_` lets it infer the element
type.

`.copied()` shows up whenever you iterate a collection of `Copy` values and
want the values rather than references to them. `.cloned()` is the same idea for
non-`Copy` types, at the cost of an allocation each.

## The three that do most of the work

| Method | Job |
|--------|-----|
| `map` | transform each item |
| `filter` | keep some items |
| `collect` | run the pipeline into a collection |

Everything else is a convenience over those. Worth knowing:

- `find_map` — first item that transforms to `Some`, short-circuiting
- `position` — index of the first match
- `any` / `all` — short-circuiting boolean tests
- `zip` — pair two iterators, stopping at the shorter
- `cycle` — repeat forever
- `enumerate` — attach an index
- `chunks` — fixed-size windows over a slice
- `min_by_key` / `max_by` — extremes by a key or a comparator

## Short-circuiting is not an optimization detail

From [`src/system.rs`](../../src/system.rs):

```rust
managers
    .iter()
    .find_map(|(program, args, label)| {
        let output = command_output(program, args);
        ...
        (count > 0).then(|| format!("{count} {label}"))
    })
```

`find_map` stops at the first `Some`. Each iteration *spawns a process*, so
this is the difference between running one package manager and running six.
Using `.map(..).filter(..).next()` would be equivalent in output and much
slower.

Note `(count > 0).then(|| ...)` — `bool::then` turns a condition into an
`Option`, which is often tidier than an `if`/`else` returning `Some`/`None`.

## `zip` and `cycle` together

From [`src/terminal/mod.rs`](../../src/terminal/mod.rs):

```rust
fn pair_with_palette<'a>(
    lines: &'a [String],
    palette: &'a [Color; PALETTE_SIZE],
) -> impl Iterator<Item = (&'a String, &'a Color)> {
    lines.iter().zip(palette.iter().cycle())
}
```

Eight colors, an unknown number of lines. `cycle` makes the palette infinite,
and `zip` stops at the shorter side — which is now always the lines. The result
is correct for one line or fifty, with no bounds check and no modulo.

This is the shape to internalize: **the right combinator often removes the edge
case entirely**, rather than handling it.

## Sorting and determinism

From [`src/palette.rs`](../../src/palette.rs):

```rust
candidates.sort_by(|left, right| {
    right.count.cmp(&left.count)                    // most common first
        .then_with(|| left.color.cmp(&right.color)) // tie-break by color
});
```

`right.cmp(&left)` rather than `left.cmp(&right)` is how you sort descending.

The `then_with` is not cosmetic. These candidates came out of a `HashMap`,
whose iteration order is deliberately unspecified and varies between runs. With
only the count comparison, two equally common colors could swap places from run
to run and the same sprite would produce different palettes. The tie-break is
what makes `extraction_is_deterministic_for_one_sprite` pass.

For floats you cannot use `cmp` at all, because `f64` is not `Ord` — NaN is not
ordered with respect to anything. `total_cmp` is the escape hatch:

```rust
.max_by(|left, right| seed_score(**left, &centers).total_cmp(&seed_score(**right, &centers)))
```

## When not to use a chain

```rust
for (x, y, pixel) in source.enumerate_pixels() {
    if pixel[3] > 0 {
        found = true;
        left = left.min(x);
        ...
    }
}
```

[`crop_transparency`](../../src/image_ops.rs) updates five variables at once. A
`fold` carrying a five-tuple would be shorter and much worse to read. A plain
loop is idiomatic Rust when the loop is doing genuinely stateful work.

The chain is not the goal. Clarity is.

---

Previous: [Traits](04-traits.md) · Next: [Modules and visibility](06-modules.md)
