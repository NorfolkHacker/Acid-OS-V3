# Acid OS v3: under the hood

This is the developer's guide to the code. If you just want to run Acid OS or
write apps for it, start with the [main README](../README.md) and the
[manual](../docs/manual-v3/README.md).

Run every command here from the **top folder of the repository**, not from
inside `v3/`. The OS finds its apps through paths like `v3/apps/...`.

## What's here

The OS is a Cargo workspace. Most of it is `no_std`, so the core can move to
small hardware later; only the Lua host and the desktop backend need the
standard library.

| Folder | What it is |
| --- | --- |
| `crates/acid-platform` | The traits every backend implements |
| `crates/acid-gfx` | Drawing: RGB565 canvases, circles, the font, the wallpaper |
| `crates/acid-kernel` | Windows, input routing, the compositor, starting apps |
| `crates/acid-synth` | The synthesiser |
| `crates/acid-api` | The `acid_*` calls apps make, independent of Lua or WASM |
| `crates/acid-lua` | Runs each app in its own Lua 5.4 VM |
| `crates/acid-wasm` | Runs WASM carts under wasmi, with memory and CPU limits |
| `crates/acid-hosted` | The Linux desktop backend: threads, a window, sound |
| `crates/acid-os` | Boots the system; this is the program you run |
| `crates/acid-testkit` | A fake platform so tests can run without a screen |
| `apps/` | The built-in Lua apps; `apps/lib/` holds code they share |
| `carts/` | Carts you can install with Load Cart |
| `carts-src/` | Rust source for WASM carts (a separate workspace) |
| `fsroot/` | The filesystem the OS sees |
| `tools/` | Lua test scripts and data generators |

## Running

```sh
cargo run --release --manifest-path v3/Cargo.toml -p acid-os
```

Add `-- --app tetris` to open an app straight away. Any app in `v3/apps/` with
an `.app.toml` manifest works, using the file's name (`tetris`, `piano`,
`file_manager` and so on). Setting `menu = false` in a manifest hides that app
from the Menu.

To check your sound works, this plays a short arpeggio:

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-hosted --example beep
```

## Carts

A cart is an app you install yourself with the Load Cart app, rather than one
that ships with the OS. Load Cart looks in `v3/carts`, `~/carts` and on
removable media. Installed carts are less trusted than built-in apps: they can
only write files under `Home`, and they can't close or rearrange other
windows.

There are two kinds:

- **`.cart` files** are Lua programs.
- **`.wasm` files** are WebAssembly modules. They always run in the stricter
  cart sandbox, with their memory and CPU capped.

### Building a WASM cart

WASM carts are written in Rust using the `acid-cart` crate in
`carts-src/acid-cart`. You implement its `Cart` trait, name your type in the
`acid_cart!` macro, and put the cart's header in a
`#[unsafe(link_section = "acid")]` static. Load Cart reads that header to
learn the cart's name and settings.

`carts-src` is its own workspace, so building the OS never needs the
WebAssembly target. To build carts, install it first
(`rustup target add wasm32-unknown-unknown`, or `sudo pacman -S rust-wasm` on
Arch). Then, to build the example and copy it where Load Cart will find it:

```sh
cargo build --manifest-path v3/carts-src/Cargo.toml --release \
  --target wasm32-unknown-unknown -p hello-wasm
cp v3/carts-src/target/wasm32-unknown-unknown/release/hello_wasm.wasm v3/carts/
```

The [manual's WASM chapter](../docs/manual-v3/10-wasm-carts.md) covers the
details.

## Testing

```sh
cargo test --manifest-path v3/Cargo.toml --workspace
```

You'll need pkg-config and the ALSA development package (`libasound2-dev`,
`alsa-lib-devel` or `alsa-lib`, depending on your distro), plus the WebAssembly
target, because the tests build the example cart from source. If the target is
missing, that one test fails and tells you why.

A few things the tests check that are worth knowing about:

- **The screen, pixel for pixel.** `acid-os` compares what it draws against
  reference frames committed to the repository.
- **The synth, byte for byte.** `acid-synth` plays a script and compares the
  output against a committed recording.
- **The manual.** Every complete example in `docs/manual-v3/` runs as a test,
  and every snippet is checked to parse. Run just those with
  `cargo test --manifest-path v3/Cargo.toml -p acid-os --test manual`.

The reference frames and the synth recording are the source of truth. If one
of those tests fails, fix the code. Don't regenerate the reference to make the
test pass.
