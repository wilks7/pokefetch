# A tour of Pokefetch, for people learning Rust

Pokefetch is a real, working terminal program — roughly 5,000 lines of Rust
across a library, two binaries, a build script, and a test suite. It does
something you can see, which makes it a better place to read Rust than a set of
isolated examples.

A fair warning about that number: a good chunk of it is comments and doc
comments, deliberately. The code was annotated for this tour.

This tour walks the codebase in an order that builds up, rather than in
alphabetical order. Each chapter points at real functions you can open, change,
and re-run.

## Before you start

```sh
cargo run -- show pikachu     # run it
cargo test                    # 79 tests, all offline
cargo doc --open              # browse the API docs generated from the source
```

`cargo doc --open` is worth doing early. Every module in this crate starts with
a `//!` comment listing the Rust concepts it demonstrates, and those render into
a browsable site. The tour and the API docs are meant to be read side by side.

## The chapters

| # | Chapter | What you will learn |
|---|---------|---------------------|
| 1 | [Orientation](01-orientation.md) | How a Rust project is laid out, and how this one runs |
| 2 | [Ownership and borrowing](02-ownership.md) | The idea that makes Rust different, in code that needed it |
| 3 | [Errors and absence](03-errors.md) | `Result`, `Option`, `?`, and when to stop caring about an error |
| 4 | [Traits and generics](04-traits.md) | `From`, `Default`, `impl Trait`, and what `derive` actually does |
| 5 | [Iterators](05-iterators.md) | Reading long `.map().filter().collect()` chains without fear |
| 6 | [Modules and visibility](06-modules.md) | `mod`, `pub`, `pub(crate)`, and how files become a tree |
| 7 | [Testing](07-testing.md) | Three kinds of test, all of which this crate uses |
| 8 | [The build script](08-build-script.md) | Generating Rust at compile time |
| 9 | [Exercises](09-exercises.md) | Changes to make, in rough order of difficulty |

## How to use this

Read a chapter, then open the file it names and read the real thing. The code
is commented for exactly this purpose, but the comments assume you have the
context a chapter gives you.

If you only do one thing: go to [Exercises](09-exercises.md) and make the first
change. Reading Rust and writing Rust teach different things, and the compiler
is a better teacher than any document.

## What this tour is not

It is not a replacement for [the Rust Book](https://doc.rust-lang.org/book/),
which explains the language properly and in order. This tour assumes you are
reading that (or have) and want to see the ideas in something that runs.
