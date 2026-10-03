# Acid OS v3

Acid OS is a tiny operating system with a desktop, a handful of apps and
some games, written in Rust. Right now it runs in a window on Linux, so you
can try it without flashing anything. The long-term plan is to run it on
microcontrollers and small single-board computers too.

Apps are written in Lua. Each one gets its own window and runs separately
from the others, so a buggy app can't take the rest of the system down with
it. You can also write apps (called "carts") in Rust or anything else that
compiles to WebAssembly. Those run in a stricter sandbox, with limits on how
much memory and CPU they can use.

Out of the box you get a terminal, a file manager, a text editor, a system
monitor and a few settings apps, plus Tetris, Breakout, Acid Blaster and a
little piano.

## Trying it out

You'll need a recent Rust (1.85 or newer) and the ALSA sound library
headers. On Debian or Ubuntu that's `libasound2-dev`, on Fedora it's
`alsa-lib-devel`, and on Arch it's `alsa-lib`.

Then, from the top folder of this repository:

```sh
cargo run --release --manifest-path v3/Cargo.toml -p acid-os
```

That boots you to the desktop. Open apps from the Menu.

If you want to jump straight into a game, name it at the end:

```sh
cargo run --release --manifest-path v3/Cargo.toml -p acid-os -- --app tetris
```

Make sure you run these from the top folder, not from inside `v3/`.
Otherwise the OS can't find its apps.

## Writing your own apps

The [manual](docs/manual-v3/README.md) walks you through it, from a
"hello world" window up to graphics, sound, games and WebAssembly carts.
Every example in it is checked by the test suite, so the code should work
as written.

## Running the tests

```sh
cargo test --manifest-path v3/Cargo.toml --workspace
```

The tests build a sample WebAssembly cart, so you'll also need the
WebAssembly target installed (`rustup target add wasm32-unknown-unknown`,
or `sudo pacman -S rust-wasm` on Arch).

## Under the hood

If you want to know how it's put together, how carts are built, or what the
tests check, read
[`v3/README.md`](v3/README.md).

## License

Acid OS is MIT licensed (see [`LICENSE`](LICENSE)). The bitmap font comes
under a BSD license; see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
