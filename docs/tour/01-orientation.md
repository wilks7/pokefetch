# 1. Orientation

## Running it

```sh
cargo run -- show pikachu
```

Everything after `--` goes to the program rather than to Cargo. Without a
subcommand you get the full greeting:

```sh
cargo run
```

If your terminal cannot draw images you will see five lines of colored text
instead of a sprite. That is not a failure — see [`terminal::detect`](../../src/terminal/detect.rs).

## The layout

```text
pokefetch/
├── Cargo.toml          project manifest: dependencies, features, lints
├── build.rs            runs BEFORE compilation, generates code
├── src/
│   ├── lib.rs          the library crate root
│   ├── main.rs         the executable (30 lines, delegates to the library)
│   ├── cli.rs          command-line surface
│   ├── app.rs          command dispatch
│   ├── config.rs       settings and validation
│   ├── palette.rs      color extraction
│   ├── sprite.rs       finding sprite bytes
│   ├── image_ops.rs    cropping, scaling, encoding
│   ├── icon.rs         macOS .icns generation
│   ├── system.rs       cached machine facts
│   ├── pokemon/        species lookup (a module as a directory)
│   ├── terminal/       terminal detection, Kitty protocol, layout
│   └── bin/            a second executable
├── tests/              integration tests (separate crates)
├── assets/, sprites/   artwork
├── manifests/          which artwork goes into which build
└── shell/              Fish integration
```

Two conventions worth noticing immediately:

**`src/lib.rs` and `src/main.rs` are both special names.** Cargo treats
`lib.rs` as a library crate and `main.rs` as a binary crate. This project has
both, which is common for CLI tools: the library holds the logic, the binary is
a thin wrapper. Open [`src/main.rs`](../../src/main.rs) — it really is that
short.

**A module can be a file or a directory.** `config.rs` is one file.
`pokemon/` is a directory with `mod.rs` (the module itself) and `names.rs` (a
submodule). Both are just `pokemon` to the rest of the code. Split when a file
gets long, not because a rule says to.

## Why the library/binary split

It buys three concrete things:

1. **`cargo doc` documents your own code.** A binary-only crate produces
   nothing useful.
2. **Doc comments become tests.** The examples in `///` comments are compiled
   and run by `cargo test`. Try breaking one — change the expected value in the
   example on `Pokemon::label` and watch `cargo test` fail. Documentation that
   lies fails the build.
3. **`tests/` can exist.** Files in `tests/` link against the library as an
   outside user would, so they can only touch `pub` items.

## Following one command through

`pokefetch show pikachu` goes:

```text
main.rs          Cli::parse()          arguments -> a Cli struct
  app.rs         run(cli)              dispatch on the subcommand
    config.rs    Config::load()        read TOML, or use defaults
    cli.rs       apply_overrides()     flags win over the file
    config.rs    validate()            is this drawable?
    pokemon/     resolve()             "pikachu" -> #025
    sprite.rs    SpriteStore::resolve() find the pixels
    image_ops.rs render_square()       crop, scale, center
    palette.rs   extract()             eight colors
    terminal/    print_greeting()      draw it
```

Open [`src/app.rs`](../../src/app.rs) and read `run`. Every subcommand is one
arm of a `match`, and they all follow the same three beats.

## Cargo commands you will use

```sh
cargo run -- <args>     build and run
cargo test              run every test
cargo clippy            the linter; this project treats its warnings as errors
cargo fmt               format; there is one correct style and this is it
cargo doc --open        build and open API documentation
cargo build --release   optimized build (slower to compile, faster to run)
```

`cargo fmt` is worth internalizing early. Rust communities do not argue about
formatting because the formatter settles it.

---

Next: [Ownership and borrowing](02-ownership.md)
