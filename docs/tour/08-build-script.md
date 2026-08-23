# 8. The build script

[`build.rs`](../../build.rs) at the project root is special: Cargo compiles and
runs it **before** compiling your crate. It is ordinary Rust that runs at build
time.

Pokefetch uses one to bake sprites into the executable, which is what makes an
offline build possible.

## What it produces

The script reads the TOML manifests, decides which sprites the requested bundle
needs, and writes Rust source:

```rust
struct Asset {
    game: &'static str,
    variant: &'static str,
    species: &'static str,
    bytes: &'static [u8],
    palette: [(u8, u8, u8); 8],
}
static ASSETS: &[Asset] = &[
    Asset { game: "red-blue", variant: "front", species: "1",
            bytes: include_bytes!("/abs/path/1.png"),
            palette: [(120, 200, 80), ...] },
    ...
];
```

`include_bytes!` embeds a file's contents as a `&'static [u8]` at compile time.
That is why a bundled binary needs no sprite files at runtime — the PNGs are
*inside* it.

## How it gets used

[`src/sprite.rs`](../../src/sprite.rs) pulls the generated file in:

```rust
mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundled.rs"));
}
```

Three macros, all resolved at compile time:

- `env!("OUT_DIR")` — the directory Cargo gave the build script to write into
- `concat!` — joins string literals
- `include!` — pastes the file's contents in as if you had typed them

## Talking to Cargo

A build script communicates by printing:

```rust
println!("cargo:rerun-if-changed=manifests/sets.toml");
println!("cargo:rerun-if-env-changed=POKEFETCH_BUNDLE");
```

These declare dependencies. Without them Cargo re-runs the script on every
build, or worse, fails to re-run it when a sprite changes and you get a stale
binary. Getting these right is most of what makes an incremental build correct.

## Sharing code with a build script

Here is a genuine constraint worth understanding:

```rust
#[path = "src/palette.rs"]
#[allow(dead_code)]
mod palette;
```

A build script is compiled as its **own separate crate**, before your library
exists. So it cannot `use pokefetch::palette` — there is nothing to use yet.

`#[path]` points a `mod` declaration at an arbitrary file, so the same source
gets compiled a second time, into the build script. It is the standard workaround.

Two consequences:

- `src/palette.rs` cannot use `crate::` paths, or it would break here. Check it —
  it does not.
- `#[allow(dead_code)]` is needed because the build script uses only part of the
  module, and unused-code warnings would otherwise fire.

Contrast with [`src/bin/pokefetch-assets.rs`](../../src/bin/pokefetch-assets.rs),
which is a *binary* target and therefore can depend on the library normally:

```rust
use pokefetch::palette;
```

Same crate, different rules, because of when each one is compiled.

## `panic!` is the correct error handling here

The build script uses `expect` and `assert!` where the rest of the crate uses
`anyhow`:

```rust
assert_eq!(catalog.schema_version, 1, "unsupported set catalog schema");
```

A build script has no user to apologize to and nothing to degrade into. A
malformed manifest should stop the build loudly. Matching your error strategy to
your audience is the point — the library's audience is a person at a prompt, the
build script's audience is whoever just broke the manifest.

## Cargo features

[`Cargo.toml`](../../Cargo.toml):

```toml
[features]
default = []
bundle-assets = []
```

Features are compile-time flags. Cargo exposes each enabled one to the build
script as `CARGO_FEATURE_<NAME>`, and to your code as `#[cfg(feature = "...")]`.

```sh
cargo build                                              # no bundle, stub functions
POKEFETCH_BUNDLE=red-blue-core cargo build --features bundle-assets
POKEFETCH_BUNDLE=retro-master cargo build --features bundle-assets
```

Notice the design choice: with no feature, the build script still emits a
`bundled` module — one whose functions always return `None`. So `src/sprite.rs`
contains no `#[cfg]` at all. **The conditional compilation happens in one place,
and the rest of the crate is written as if bundles always exist.**

That is worth copying. Scattering `#[cfg(feature = ...)]` through your logic is
how a codebase acquires configurations nobody ever compiles.

## Cost

Build scripts slow down builds — this one reads a manifest of 2,362 assets and
emits an `include_bytes!` for each selected one. Reach for a build script when
you genuinely need work done at compile time: embedding assets, generating
bindings, or querying the target platform.

Worth noticing how the cost was kept down. Palettes are *not* computed here;
they were computed once by `pokefetch-assets` at import time and recorded in
`assets/manifest.toml`, so the build script only parses hex strings. An earlier
version of this script decoded and quantized all 151 sprites on every build.

---

Previous: [Testing](07-testing.md) · Next: [Exercises](09-exercises.md)
