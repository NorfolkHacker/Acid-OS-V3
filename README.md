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

Out of the box you get:

- a terminal, a file manager, a text editor, a system monitor, network and
  settings apps;
- Sprite Paint, for pixel art and animated sprites;
- Acid Tracker, a four-channel tracker for `.trk` songs, with its own audio
  scripting language (`.snd`);
- Tetris, Breakout, Acid Blaster, a little piano, and a spinning 3D demo.

## Getting started

### 1. Install what it needs

You need:

- Rust 1.85 or newer;
- a C compiler and `pkg-config`, because the bundled Lua and the sound
  library are built from source;
- the ALSA sound library headers;
- `git`;
- a Linux desktop session (X11 or Wayland) to show the window in.

On **Debian / Ubuntu**:

```sh
sudo apt install build-essential pkg-config libasound2-dev git curl
```

On **Fedora**:

```sh
sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel git curl
```

On **Arch**:

```sh
sudo pacman -S --needed base-devel alsa-lib git curl
```

Then install Rust with [rustup](https://rustup.rs). Skip this if `cargo --version`
already says 1.85 or newer.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2. Get the code

```sh
git clone https://github.com/NorfolkHacker/Acid-OS-V3.git
cd Acid-OS-V3
```

### 3. Run it

From the top folder of the repository, not from inside `v3/`:

```sh
cargo run --release --manifest-path v3/Cargo.toml -p acid-os
```

The first build downloads and compiles everything, so it takes a few
minutes. Later runs start in seconds.

Acid OS opens on a **screen-size picker**: 640×480 (the default), 640×360 or
800×600. Pick one with the arrow keys and Enter or a click, or wait three
seconds for the default. Then the desktop appears. Open apps from the **Menu**
at the top left. Games, demos, Sprite Paint and Acid Tracker open from the **File Manager**:
open them from the File Manager's **Apps** or **Games** folder.

### Options

Add options after `--` at the end of the command.

| Option | What it does |
|---|---|
| `--screen 640x480` | Skips the picker and uses that size. The others are `640x360` and `800x600`. |
| `--app <name>` | Opens an app straight away, as well as the desktop. |

These are the app names you can use with `--app`:

| Kind | Names |
|---|---|
| Games | `tetris`, `breakout`, `acid_blaster`, `acidstorm`, `acid_snake`, `acid_invaders`, `acid_rocks` |
| Toys | `piano`, `acid_spin`, `sprite` (Sprite Paint), `tracker` (Acid Tracker), `hello_acid` |
| Tools | `terminal`, `editor`, `file_manager`, `sysmon`, `network`, `config`, `about`, `cart` (Load Cart) |

For example:

```sh
cargo run --release --manifest-path v3/Cargo.toml -p acid-os -- --screen 800x600 --app tetris
```

### Quitting and restarting

- **To quit:** close the Acid OS window, or press Ctrl+C in the terminal you
  started it from.
- **To go back to the screen-size picker:** open **Config** and press
  **RESTART** twice. Every open app closes without saving.

### Running the tests

The tests build a sample WebAssembly cart, so add that target first. Do
this once:

```sh
rustup target add wasm32-unknown-unknown
```

If your Rust came from Arch's `rust` package instead of rustup, run
`sudo pacman -S rust-wasm` instead.

Then run the tests:

```sh
cargo test --manifest-path v3/Cargo.toml --workspace
```

### If something goes wrong

- **The desktop doesn't appear, or apps won't open.** Run the command from the
  top folder of the repository, not from inside `v3/`.
- **The build fails mentioning `alsa`, `pkg-config` or `cc`.** One of the
  packages in step 1 is missing.
- **There's no sound.** If no audio device is available, Acid OS prints
  `running silently` and carries on without it.
- **`cargo` is not found.** Open a new terminal, or run
  `source "$HOME/.cargo/env"`, after installing rustup.

## Writing your own apps

The [manual](docs/manual-v3/README.md) walks you through it, from a
"hello world" window up to graphics, sound, games and WebAssembly carts.
Every example in it is checked by the test suite, so the code should work
as written.

## Under the hood

If you want to know how it's put together, how carts are built, or what the
tests check, read
[`v3/README.md`](v3/README.md).

## License

Acid OS is MIT licensed (see [`LICENSE`](LICENSE)). The bitmap font comes
under a BSD license; see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
