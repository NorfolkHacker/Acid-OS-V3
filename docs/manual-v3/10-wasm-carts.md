# 10. WASM carts

[← API reference](09-api-reference.md) · [Contents](README.md) · [Next: Music →](11-music.md)

A **WASM cart** is an app written as a WebAssembly module instead of a Lua
script. WebAssembly ("WASM") is a compact program format that many languages,
such as Rust and C, can compile to.

A WASM cart gets the same kind of window as a Lua app and can do the same
things: draw, play sound, manage windows, use files. Acid OS runs it with
`wasmi`, a WebAssembly interpreter.

What changes is the shape of the program:

- A Lua app runs its own event loop and calls `acid_poll_event`.
- A WASM cart is a set of **callbacks**. The host (Acid OS) runs the loop and
  calls them.
- The operations reach the cart as **imports** from a module named `"acid"`,
  instead of Lua globals.

This chapter describes **version 1** of that interface (the ABI, or
application binary interface).

You'll learn about the callbacks, how values pass between the cart and the
host, the limits, the header and installing, writing a cart in Rust with the
`acid-cart` crate, and writing one by hand in the WebAssembly text format. The
full list of imports is at the end, in [The import table](#the-import-table).

## 10.1 What a WASM cart is

- **It is always cart-level.** Only `.lua` scripts can be trusted as built-in
  apps, so every `.wasm` module follows the cart rules of
  [§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps) and
  [§7.11](07-system-apis.md#711-what-a-cart-is-refused), however it was
  started. It can write only under `v3/fsroot/Home/`, can't close or raise
  other windows, and can't open the overlay.
- **It installs like a `.cart`**, through Load Cart. It lands as
  `v3/apps/<slug>.wasm`, with a manifest that says `runtime = wasm`
  ([§2.7](02-apps-and-manifests.md#27-wasm-carts)).
- **The module format itself is the sandbox.** A WASM cart can touch only its
  own block of memory (its "linear memory") and the imports in the table. A
  module that imports anything else is refused before it runs.

**When to choose WASM over Lua.** Lua is the default, and the right choice for
almost every app:

- there is nothing to install or compile, because an app is just a text file;
- the libraries (`AcidApp`, `AcidGame`, `AcidPalette`) do the bookkeeping for
  you;
- you can change the app with the Editor while the OS is running.

Choose WASM when:

- you already have the code in Rust, C or another language that targets
  WebAssembly;
- you want a compiled language's types and tooling;
- you want a cart whose source is not shipped with it.

WASM is not a way around the cart rules, since it is always cart-level. And
because it runs in an interpreter, it isn't automatically faster than Lua.

## 10.2 The callbacks

Your cart **exports** these functions so the host can call them. Before calling
any of them, the host checks they all exist and have the right types. If a
required one is missing or has the wrong signature, the module is refused. (If
the module has a start function, that has already run by this point.)

| Export | Required | Purpose |
|---|---|---|
| `memory` | yes | the cart's linear memory |
| `acid_abi_version() -> i32` | yes | must return `1`, or the cart is refused with a log line |
| `acid_on_create()` | yes | called once, then `acid_redraw` |
| `acid_on_event(kind, a, b, c)` | yes | one input event; the kinds are below |
| `acid_on_idle()` | yes | each poll timeout with no event |
| `acid_redraw()` | yes | on create, and after a "moved" or "resized" event |
| `acid_poll_timeout_ms() -> i32` | no | asked before each poll; 200 if not exported |
| `acid_on_destroy()` | no | once, after the close event; then the cart ends |

All parameters and results are `i32`.

**The event kinds** passed to `acid_on_event`:

| `kind` | Event | `a`, `b`, `c` | Then |
|---|---|---|---|
| 1 | touch | `x`, `y`, `pressed` (0/1) | |
| 2 | key | `code`, `pressed` (0/1), 0 | |
| 3 | moved | 0, 0, 0 | the host calls `acid_redraw` |
| 4 | close | 0, 0, 0 | the host calls `acid_on_destroy`, if exported, and the cart ends |
| 5 | resized | `w`, `h`, 0 | the host calls `acid_redraw` |

Touch coordinates are relative to your window, and key codes are the same ones
a Lua app gets ([§3.1](03-app-lifecycle.md#31-the-callbacks)). Ignore any kind
you don't recognise, because a later version of the ABI may add new ones.

**The host runs the loop.** In order, it:

1. instantiates the module (running its start function, if it has one);
2. calls `acid_abi_version`, and refuses the cart unless it returns 1;
3. calls `acid_on_create`, then `acid_redraw`;
4. then, until close: asks `acid_poll_timeout_ms` (if exported), waits that long
   for an event, and calls `acid_on_event` for an event or `acid_on_idle` for a
   timeout.

The poll timeout is clamped to **1–60,000 ms**. Anything below 1 counts as 1,
and anything over a minute counts as a minute.

A cart never polls for events itself. **Nothing is redrawn for it, either.**
After a touch that changes what the window shows, the cart must draw the change
itself, usually by calling its own redraw.

As in Lua, `notify_redraw_done` does nothing in this version, and "moved"
events are not sent
([§3.2](03-app-lifecycle.md#moved-and-acid_notify_redraw_done)). `acid-cart`
still calls it after the redraw that follows a move, so a cart built with it
won't need changing if "moved" events come back.

## 10.3 Passing values

Every import lives in the module `"acid"`. Each one has the same name and
meaning as the Lua call, minus the `acid_` prefix, so `fill_rect` is
`acid_fill_rect` and `get_volume` is `acid_get_volume`. The only Lua call with
no import is `acid_poll_event`, because the host runs the loop.

**Numbers.**

- Integers and colours are `i32` (32-bit integers). For a colour, the low 24
  bits are read as `0xRRGGBB`.
- Booleans are `i32` `0` or `1`.
- Three results are `i64` (64-bit): `now_ms`, `mem_used_kb` and `fs_size`.
- Window, launcher and task indexes start at 0, as in Lua.

**Strings going in** are passed as **`(ptr, len)`**: where the string starts
in the cart's memory (a byte offset), and how many bytes long it is. If that
range isn't inside memory, or the bytes aren't valid UTF-8, the import
**traps**: it stops with an error, and the cart ends. File contents passed to
`fs_write` are raw bytes, so they don't have to be UTF-8.

**Strings coming out** are written into a buffer the cart provides,
**`(buf, cap)`**: where the buffer starts and how big it is.

- The call writes `min(len, cap)` bytes at `buf`, and **returns the full
  length** `len`.
- So if the result is longer than `cap`, you get the first `cap` bytes, and
  you can try again with a bigger buffer.
- A negative return is an error code.
- The whole `(buf, cap)` range must be inside memory, even when the result
  would fit in less. Otherwise the call traps.
- `fs_read` and `cart_read` write file bytes, which don't have to be UTF-8.

**Records.** Calls that answer with several values (`window_info`,
`local_time`, `network_info`, `task_info`, `cart_stat`) write a record:

> A call with several results writes them as one UTF-8 record, its fields
> joined by tab (`\t`) characters, into a cart-supplied `(ptr, cap)` buffer by
> the string-result rule above, and returns the record's full length, or −1
> when there is no such thing (for example, no window at that index). Numbers
> are written in decimal and booleans as `0` or `1`.

For example, say the sample cart from Load Cart is open, focused, 200 × 150,
at (40, 60). `window_info` for it writes
`v3/apps/hello_wasm.wasm\t40\t60\t200\t150\t1` and returns 39. Note that
the first field is the window's script path, not its title.

`launcher_path` and `launcher_name` follow the same rule with a single field.
−1 means there's no app at that index.

**Lists.** `fs_list`, `cart_roots` and `cart_list` write their names
separated by `\n` (newline), with no newline at the end. An empty folder gives
an empty string (length 0).

**Error codes.** Where the Lua version of a file or host-cart-folder call would
return `nil, message`, the import returns a negative code:

| Code | Lua message | Meaning |
|---|---|---|
| −1 | `not found` | no such file or directory |
| −2 | `bad path` | the path guard rejected the path ([§7.8](07-system-apis.md#the-path-guard)) |
| −3 | `read only` | a write, rename or delete outside `v3/fsroot/Home/` |
| −4 | `not allowed` | a call carts may not make: the `cart_*` calls, and `close_window` |
| −5 | `too big` | a host cart file over 256 KB |
| −6 | anything else | any other failure (the platform's own message) |

`fs_write`, `fs_rename` and `fs_delete` return 0 when they succeed. `fs_size`
returns the size as an `i64`, or an error code.

**Refusals that are not error codes.** The cart rules of
[§7.11](07-system-apis.md#711-what-a-cart-is-refused) apply exactly as in Lua.
Where the Lua call returns `false`, the import returns `0`:

- `overlay_open` always returns 0.
- `launcher_register` always returns 0.
- `spawn_app` returns 0 for a path outside `v3/apps/`.
- `spawn_app` and `launcher_spawn` also return 0:
  - when the app is single-instance and already open (a built-in app would
    raise that window, but a cart may not, so nothing is raised or focused);
  - while 4 or more cart-level windows are open, counting the cart's own;
  - once the cart's own window has been closed.

An app that a cart starts runs cart-level, even a `.lua` under `v3/apps/` that
would otherwise be built-in. A built-in caller never raises a copy of a
singleton that a cart started. It opens a trusted window of its own instead.

`activate_window` does nothing unless the index is the cart's own window.

**The one exception is `close_window`.** A cart can never close a window, and
it gets **−4**, not 0.

`set_volume` and `set_wallpaper_enabled` work just as they do for any app.

## 10.4 Limits

A WASM cart has no Lua memory limit and no "stopped responding" clock. Instead,
it is kept in check by **fuel** (a budget of work per callback) and by a cap on
its **memory pages**.

| Limit | Value |
|---|---|
| Fuel per callback (`WASM_FUEL`) | 200,000,000 |
| Linear memory (`WASM_MAX_PAGES`) | 256 pages of 64 KiB: 16 MB |
| Tables | 1, of at most 65,536 elements |
| Instances, memories | 1 each |
| Module file (Load Cart) | 256 KB |

### Fuel

Every callback starts with a **fresh budget of 200,000,000 fuel**, roughly a
second of interpreted code. Each WebAssembly instruction uses up some fuel, and
**running out ends the cart**.

Starting the module up costs fuel too. A start function gets a budget of its
own, so an endless loop there ends the cart just like one in a callback.

**Work the host does for you costs fuel as well**, at **1 fuel per 8 bytes**
(rounded up). Without this, an import call would cost the same whether it
moved one byte or 16 MB, and a cart could keep the host busy far longer than
its budget allows. The host charges for:

- **every byte copied between the cart and the host**: any string or data
  you pass in, and the whole result of a strings-out call. You pay for the
  whole result even when `cap` is smaller, because the host has already made
  it. So asking for just the length with `cap` 0 costs as much as reading it;
- **every draw, by the area it covers**, at 2 bytes per pixel. Each side is
  first clamped to the screen, and the charge is made before
  drawing:
  - `fill_rect`, `overlay_fill_rect` and `repaint_region`: `w × h`;
  - `fill_circle`: the bounding square, `(2r + 1)²`;
  - `draw_text`: one 6 × 8 glyph cell per byte, on top of the string's bytes;
  - `draw_line`: its longer axis plus one pixel, at most the screen;
  - `fill_triangle`: its bounding box, like `fill_rect`;
  - `mesh_draw`: 64 pixels per point and 64 per face, plus each drawn edge like `draw_line` and each drawn triangle like `fill_triangle` (its bounding box's size clamped to the screen, wherever it sits, so a triangle off the screen still costs), with no cap on the total;
  - `mesh_builtin`: 64 bytes per face of the built-in (the cube has 6, the torus 72), on top of the name's bytes;
  - `mesh_new`: the `3 × n_points` and `4 × n_faces` i32s it reads, as bytes moved, plus 64 bytes per face;
  - `draw_window_frame`: a screen-wide, 16 px title bar;
  - `draw_window_border`: the screen's perimeter;
  - `clear_user_area` and `overlay_clear`: the whole screen, **76,800 fuel** at the default 640×480 (57,600 at 640×360, 120,000 at 800×600);
- **every file system and spawn call**, a flat **131,072 fuel** (the cost of
  1 MiB) on top of its bytes. These are `fs_list`, `fs_read`, `fs_size`,
  `fs_write`, `fs_rename`, `fs_delete`, `launcher_spawn` and `spawn_app`. They
  cost more because their real work, such as a trip to the disk or starting a
  new task, doesn't show in the byte count. About 1,500 of these calls fit in
  one callback, far more than a real cart needs.

Because charges are counted in pixels, a full-screen draw costs more on a bigger
screen: 640×480 costs a third more than 640×360, and 800×600 a little over
twice as much.

If a charge is more than the budget has left, the budget is emptied and the
cart ends as out of fuel, just as if an instruction had run out.

**The budget is per callback.** So, as in Lua, the way to do a lot of work is
in small slices, one per `acid_on_idle`.

### Memory

A cart's memory is capped at **256 pages (16 MB)**.

- A module that asks for more than that up front ends as out of memory before
  it runs.
- Growing past the cap with `memory.grow` does **not** return −1 to the cart.
  It **ends the cart as out of memory**, so a cart can't ignore a failed
  allocation and carry on.

Tables (WebAssembly's lists of function references) are capped the same way.
A cart gets one table of at most 65,536 elements, and growing it past that
ends the cart. Table elements are stored in host memory, outside the 16 MB.

A module that asks for a second table or a second memory is refused.

### How a cart ends

However a cart ends, its window is removed and its resources are freed. Every
case except a normal close prints one line to the terminal:

| End | Why | Terminal line |
|---|---|---|
| closed | the close event arrived, and `acid_on_destroy` (if any) returned | none |
| refused | not a valid module; a missing or mistyped export; an import not in the table; `acid_abi_version` not 1; the file could not be read | `Acid OS v3: <path>: refused: <why>` |
| stopped responding | a callback (or the start function) ran out of fuel | `Acid OS v3: <path>: stopped responding` |
| trap | a trap: `unreachable`, a division by zero, a bad string range, a Rust panic | `Acid OS v3: <path>: <trap message>` |
| out of memory | memory or a table declared or grown past its cap | `Acid OS v3: <path>: out of memory` |

A cart can't catch a trap or a fuel overrun. Other apps carry on unaffected,
because each cart runs on its own thread with its own separate state.

## 10.5 The header and installing

A `.wasm` cart keeps its header in a **custom section named `acid`** (a named
block of extra data inside the module). It holds UTF-8 text, as lines of
`key: value`, using the same keys as a `.cart` header
([§2.5](02-apps-and-manifests.md#the-header)):

```text
name: Hello WASM
w: 200
h: 150
desc: Example WASM cart -- colour-cycling bars
```

`libs` is ignored, because a WASM module doesn't load any Lua. If a module has
no `acid` section, or a broken one, it still installs, but under its filename,
at the default size (220 × 160), with no description.

To install:

1. Put the `.wasm` in `v3/carts/`, `~/carts` or a removable drive's `carts/`.
2. Open **Load Cart** from the Menu. It lists `.wasm` files alongside `.cart`
   ones, and reads each header from the custom section.
3. Install it. As for any cart, Load Cart writes the manifest first,
   `v3/apps/<slug>.app.toml` (with `runtime = wasm` before `source = cart`),
   and then `v3/apps/<slug>.wasm`
   ([§2.8](02-apps-and-manifests.md#28-where-an-installed-cart-lands)).
4. Press **RUN**, or open it from the Menu at the next boot.

There's a sample to try: `v3/carts/hello_wasm.wasm`.

## 10.6 Writing a cart in Rust with `acid-cart`

The Rust crates for carts live in `v3/carts-src/`. That folder is a Cargo
workspace of its own, so building the OS itself never needs the WebAssembly
target. `acid-cart` is `no_std` (it doesn't use Rust's standard library). It
gives you:

- **safe wrappers** for every import, at the top of the crate:
  `acid_cart::fill_rect`, `acid_cart::draw_text(text, x, y, fg, bg)` taking a
  `&str`, `acid_cart::fs_read(path, &mut buf)` returning a `Result`, and so
  on. The raw imports are in `acid_cart::sys`;
- the **`Cart` trait**, the **`Event`** and **`Error`** enums;
- the **`acid_cart!`** macro, which defines every export for you;
- a panic handler, so a panic traps and ends the cart.

### The `Cart` trait

```rust cart
pub trait Cart: Sized + 'static {
    fn new() -> Self;
    fn on_create(&mut self) {}
    fn on_event(&mut self, _e: Event) {}
    /// Each poll timeout with no event.
    fn on_idle(&mut self) {}
    fn redraw(&mut self);
    /// How long the host waits for an event before `on_idle`; values below
    /// 1 count as 1.
    fn poll_timeout_ms(&self) -> i32 {
        200
    }
    fn on_destroy(&mut self) {}
}
```

You only have to write `new` and `redraw`. The host calls `new` and then
`on_create` once (both from `acid_on_create`), then `redraw`, then `on_event`
and `on_idle` until the window closes.

### `Event` and `Error`

```rust cart
pub enum Event {
    Touch { x: i32, y: i32, pressed: bool },
    Key { code: i32, pressed: bool },
    /// The window moved; the host calls `redraw` right after this.
    Moved,
    /// The window was resized to `w` x `h`; the host calls `redraw` right after this.
    Resized { w: i32, h: i32 },
    /// The window is closing; `on_destroy` follows, then the cart ends.
    Close,
}

pub enum Error {
    NotFound,   // −1
    BadPath,    // −2
    ReadOnly,   // −3
    NotAllowed, // −4
    TooBig,     // −5
    Other,      // −6, or any code this version doesn't know
}
```

`acid_on_event` turns its four integers into an `Event`, and drops any kind it
doesn't know.

- The file and cart-folder wrappers return `Result<_, Error>`.
- Indexed records (`window_info`, `launcher_path`, `task_info`) return
  `Option<usize>`, with `None` for −1.
- **Every buffer call returns the full length**, which may be more than the
  buffer you passed. Check for that:

```rust cart
let mut buf = [0u8; 256];
match acid_cart::fs_read("v3/fsroot/Home/score.txt", &mut buf) {
    Ok(n) if n <= buf.len() => {
        let text = core::str::from_utf8(&buf[..n]).unwrap_or("");
        // ... use text
    }
    Ok(_) => { /* longer than 256 bytes: truncated */ }
    Err(acid_cart::Error::NotFound) => { /* first run */ }
    Err(_) => {}
}
```

`acid-cart` has no memory allocator, and neither does `hello-wasm`. Use
fixed-size buffers, on the stack or in your cart's struct.

`acid_cart::close_window` returns `true` only when a window was closed. A cart
is always refused (the import returns −4), so for a cart it is always `false`.

### `acid_cart!`

Name your type once, at the crate root:

```rust cart
acid_cart!(HelloWasm);
```

It defines `acid_abi_version` (returning `acid_cart::ABI_VERSION`, which is
1), `acid_on_create`, `acid_on_event`, `acid_on_idle`, `acid_redraw`,
`acid_poll_timeout_ms` and `acid_on_destroy`.

Your cart lives in a `static`. It is created in `acid_on_create` and dropped
after `on_destroy`. That is safe because a wasm32 module without threads has
only one thread, and the host calls one export at a time. It never calls back
into the cart from inside an import.

### The header

The header is a `static` placed in the `acid` custom section. Nothing in the
code refers to it, so `#[used]` stops the optimiser throwing it away. **The
array's length must match the text exactly:**

```rust cart
#[used]
#[unsafe(link_section = "acid")]
static HEADER: [u8; 78] = *b"name: Hello WASM\nw: 200\nh: 150\ndesc: Example WASM cart -- colour-cycling bars\n";
```

### `hello-wasm`, walked through

`v3/carts-src/hello-wasm` is `v3/apps/hello_acid.lua` rewritten in Rust: it
draws colour-cycling bars. After the header, it keeps its window size in
constants. These must match the header's `w` and `h`, just as a Lua app's
constants match its manifest:

```rust cart
const WINDOW_W: i32 = 200;
const WINDOW_H: i32 = 150;
const TITLE_BAR_H: i32 = 16;
const BAR_H: i32 = 8;
const TEXT_Y: i32 = 70;
```

A `hue(step)` function does the same job as `AcidPalette.hue` with its default
256 steps, and returns `0xRRGGBB`. The cart's whole state is one counter:

```rust cart
struct HelloWasm {
    step: i32,
}

impl Cart for HelloWasm {
    fn new() -> Self {
        HelloWasm { step: 0 }
    }

    // ~25 fps while focused; the host calls on_idle at this interval.
    fn poll_timeout_ms(&self) -> i32 {
        40
    }

    fn on_idle(&mut self) {
        if !acid_cart::focused() {
            return;
        }
        self.step = self.step.wrapping_add(3);
        self.redraw();
    }
```

`poll_timeout_ms` asks for an idle call every 40 ms. `on_idle` only animates
while the window has focus, as `hello_acid` does. It draws the new frame
itself, because nothing redraws a cart for it.

`wrapping_add` matters here. A plain `+` that overflowed would panic, and a
panic ends the cart.

```rust cart
    fn redraw(&mut self) {
        acid_cart::clear_user_area();
        acid_cart::draw_window_frame("Hello WASM");
        let mut y = TITLE_BAR_H;
        let mut row = 0;
        while y < WINDOW_H {
            let h = if y + BAR_H > WINDOW_H { WINDOW_H - y } else { BAR_H };
            acid_cart::fill_rect(0, y, WINDOW_W, h, hue(self.step.wrapping_add(row * 12)));
            y += BAR_H;
            row += 1;
        }
        let label_bg = hue(self.step.wrapping_add((TEXT_Y - TITLE_BAR_H) / BAR_H * 12));
        acid_cart::fill_rect(0, TEXT_Y, WINDOW_W, BAR_H, label_bg);
        acid_cart::draw_text("HELLO FROM WASM", 42, TEXT_Y, 0x050607, label_bg);
        acid_cart::draw_window_border();
    }
}

acid_cart!(HelloWasm);
```

`redraw` works just like a Lua `redraw`: clear, title bar, the bars, a label
on top of its bar, and the border last. Each frame costs about 87,000 fuel of
host work (76,800 for the clear, 400 for each bar), a tiny fraction of the
budget.

## 10.7 Building

You need Rust's standard library for the `wasm32-unknown-unknown` target. On
Arch:

```sh
sudo pacman -S rust-wasm
```

From the top folder of the repository, build the example and copy it to where
Load Cart looks:

```sh
cargo build --manifest-path v3/carts-src/Cargo.toml --release --target wasm32-unknown-unknown -p hello-wasm
cp v3/carts-src/target/wasm32-unknown-unknown/release/hello_wasm.wasm v3/carts/hello_wasm.wasm
```

Then install `hello_wasm.wasm` from Load Cart. (The test suite also builds the
example from source and checks that it runs.)

To make a cart of your own, create a new crate in `v3/carts-src/`. Add it to
the workspace's `members`, and give it a `cdylib` target and the `acid-cart`
dependency, as `hello-wasm` does:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
acid-cart = { path = "../acid-cart" }
```

Build it with `-p <crate>`. The output is
`target/wasm32-unknown-unknown/release/<crate>.wasm`, with any `-` in the crate
name turned into `_`.

The workspace's release settings (`opt-level = "s"`, LTO, `panic = "abort"`,
stripped) keep modules small. `hello_wasm.wasm` is about 1 KB, well under the
256 KB cap.

## 10.8 A cart by hand

You don't need Rust to write a cart. Here is a complete one written by hand in
the WebAssembly text format (WAT). It draws a coloured panel. Tap it and the
colour flips, and a note plays for as long as you hold it:

```wat
(module
  ;; Import only what you use; a name outside the table refuses the module.
  (import "acid" "clear_user_area"    (func $clear))
  (import "acid" "draw_window_frame"  (func $frame (param i32 i32)))
  (import "acid" "fill_rect"          (func $rect (param i32 i32 i32 i32 i32)))
  (import "acid" "draw_text"          (func $text (param i32 i32 i32 i32 i32 i32)))
  (import "acid" "draw_window_border" (func $border))
  (import "acid" "play_note"          (func $play (param i32 i32 i32)))
  (import "acid" "stop_note"          (func $stop (param i32)))

  (memory (export "memory") 1)
  (data (i32.const 0) "WAT Cart")   ;; the title: 8 bytes at 0
  (data (i32.const 16) "TAP ME")    ;; the label: 6 bytes at 16

  ;; The header Load Cart reads.
  (@custom "acid" "name: WAT Cart\nw: 200\nh: 150\ndesc: Tap to flip the colour\n")

  (global $colour (mut i32) (i32.const 0xff8800))
  (global $down (mut i32) (i32.const 0))     ;; 1 while a touch is held

  (func (export "acid_abi_version") (result i32) (i32.const 1))
  (func (export "acid_on_create"))
  (func (export "acid_on_idle"))

  (func $redraw (export "acid_redraw")
    (call $clear)
    (call $frame (i32.const 0) (i32.const 8))
    (call $rect (i32.const 20) (i32.const 40) (i32.const 160) (i32.const 60) (global.get $colour))
    (call $text (i32.const 16) (i32.const 6) (i32.const 82) (i32.const 66)
                (i32.const 0x050607) (global.get $colour))
    (call $border))

  (func (export "acid_on_event") (param $kind i32) (param $a i32) (param $b i32) (param $c i32)
    ;; Only touches (kind 1) matter; c is "pressed".
    (if (i32.ne (local.get $kind) (i32.const 1)) (then (return)))
    (if (i32.eqz (local.get $c))
      (then                       ;; release: stop the note, re-arm
        (global.set $down (i32.const 0))
        (call $stop (i32.const 0))
        (return)))
    ;; A held touch repeats "pressed" every tick: act once per hold.
    (if (global.get $down) (then (return)))
    (global.set $down (i32.const 1))
    (global.set $colour (i32.xor (global.get $colour) (i32.const 0xffffff)))
    (call $play (i32.const 0) (i32.const 49) (i32.const 60))
    (call $redraw)))
```

How it works:

- Every string is a `(ptr, len)` pointing into the cart's own memory. The
  title is the 8 bytes at offset 0, and the label is the 6 bytes at 16.
- The panel's colour lives in a global. The touch handler redraws after
  changing it, because the host won't.
- A held touch sends "pressed" on every tick, so `$down` makes the flip happen
  only once per hold. This is the pattern from
  [§3.5](03-app-lifecycle.md#35-touch-debouncing).
- A note sounds until it is stopped, so letting go stops it.

To turn it into a module, use a WAT assembler that understands the `@custom`
annotation. The Rust `wat` crate does, and the OS's own tests use it to build
this very example. `wasm-tools parse cart.wat -o cart.wasm` uses the same
parser and should work too, but it is an optional tool and isn't tested here.
An assembler that drops the annotation still makes a working cart; it just
installs under its filename at the default size.

Then install the `.wasm` from Load Cart, as above.

## The import table

Here is every import in the module `"acid"`, for ABI version 1, in order.

- All values are `i32` unless marked `i64`.
- "len" is a strings-out result: the full length, or a negative error code
  where the table says so.
- The last column is the matching Lua call in [chapter 9](09-api-reference.md).

| Import | Signature (params → result) | Notes | Lua call |
|---|---|---|---|
| `now_ms` | → i64 | | [`acid_now_ms`](09-api-reference.md#acid_now_ms) |
| `notify_redraw_done` | → | a no-op in v3 | [`acid_notify_redraw_done`](09-api-reference.md#acid_notify_redraw_done) |
| `fill_rect` | x, y, w, h, color | | [`acid_fill_rect`](09-api-reference.md#acid_fill_rect) |
| `fill_circle` | x, y, r, color | | [`acid_fill_circle`](09-api-reference.md#acid_fill_circle) |
| `draw_text` | ptr, len, x, y, fg, bg | | [`acid_draw_text`](09-api-reference.md#acid_draw_text) |
| `draw_window_frame` | ptr, len | the title | [`acid_draw_window_frame`](09-api-reference.md#acid_draw_window_frame) |
| `draw_window_border` | → | | [`acid_draw_window_border`](09-api-reference.md#acid_draw_window_border) |
| `clear_user_area` | → | | [`acid_clear_user_area`](09-api-reference.md#acid_clear_user_area) |
| `am_i_focused` | → i32 (0/1) | | [`acid_am_i_focused`](09-api-reference.md#acid_am_i_focused) |
| `launch_arg` | buf, cap → len | "" (length 0) if none | [`acid_launch_arg`](09-api-reference.md#acid_launch_arg) |
| `play_note` | voice, ona, volume | | [`acid_play_note`](09-api-reference.md#acid_play_note) |
| `stop_note` | voice | | [`acid_stop_note`](09-api-reference.md#acid_stop_note) |
| `configure_voice` | voice, route, attack, decay, sustain, release | | [`acid_configure_voice`](09-api-reference.md#acid_configure_voice) |
| `configure_filter` | cutoff, resonance, mode | | [`acid_configure_filter`](09-api-reference.md#acid_configure_filter) |
| `trigger_arp` | voice, n0, n1, n2, n3, count, rate_ms | the four notes as separate arguments | [`acid_trigger_arp`](09-api-reference.md#acid_trigger_arp) |
| `configure_osc` | voice, waveform, duty | | [`acid_configure_osc`](09-api-reference.md#acid_configure_osc) |
| `set_ring_partner` | voice, partner | | [`acid_set_ring_partner`](09-api-reference.md#acid_set_ring_partner) |
| `set_volume` | percent | open to carts | [`acid_set_volume`](09-api-reference.md#acid_set_volume) |
| `get_volume` | → i32 | | [`acid_get_volume`](09-api-reference.md#acid_get_volume) |
| `active_voice_count` | → i32 | | [`acid_active_voice_count`](09-api-reference.md#acid_active_voice_count) |
| `overlay_open` | → i32 (0/1) | always 0 for a cart | [`acid_overlay_open`](09-api-reference.md#acid_overlay_open) |
| `overlay_clear` | → | acts only for the overlay's holder | [`acid_overlay_clear`](09-api-reference.md#acid_overlay_clear) |
| `overlay_fill_rect` | x, y, w, h, color | acts only for the overlay's holder | [`acid_overlay_fill_rect`](09-api-reference.md#acid_overlay_fill_rect) |
| `overlay_close` | → | acts only for the overlay's holder | [`acid_overlay_close`](09-api-reference.md#acid_overlay_close) |
| `repaint_region` | x, y, w, h | | [`acid_repaint_region`](09-api-reference.md#acid_repaint_region) |
| `set_wallpaper_enabled` | on (0/1) | open to carts | [`acid_set_wallpaper_enabled`](09-api-reference.md#acid_set_wallpaper_enabled) |
| `get_wallpaper_enabled` | → i32 (0/1) | | [`acid_get_wallpaper_enabled`](09-api-reference.md#acid_get_wallpaper_enabled) |
| `window_max` | → i32 | | [`acid_window_max`](09-api-reference.md#acid_window_max) |
| `screen_w` | → i32 | screen width in pixels | [`acid_screen_size`](09-api-reference.md#acid_screen_size) |
| `screen_h` | → i32 | screen height in pixels | [`acid_screen_size`](09-api-reference.md#acid_screen_size) |
| `font_w` | → i32 | this window's character width in pixels | [`acid_font_size`](09-api-reference.md#acid_font_size) |
| `font_h` | → i32 | this window's character height in pixels | [`acid_font_size`](09-api-reference.md#acid_font_size) |
| `window_w` | → i32 | this window's width in pixels | [`acid_window_size`](09-api-reference.md#acid_window_size) |
| `window_h` | → i32 | this window's height in pixels | [`acid_window_size`](09-api-reference.md#acid_window_size) |
| `get_font_scale` | → i32 (1 or 2) | Config's font setting | [`acid_get_font_scale`](09-api-reference.md#acid_get_font_scale) |
| `set_font_scale` | n (1 or 2) | anything else is ignored | [`acid_set_font_scale`](09-api-reference.md#acid_set_font_scale) |
| `draw_line` | x1, y1, x2, y2, color | | [`acid_draw_line`](09-api-reference.md#acid_draw_line) |
| `fill_triangle` | x1, y1, x2, y2, x3, y3, color | | [`acid_fill_triangle`](09-api-reference.md#acid_fill_triangle) |
| `mesh_builtin` | name_ptr, name_len → i32 | the mesh id; −1 for an unknown name (checked before the limits), else −5 over the limits | [`acid_mesh_builtin`](09-api-reference.md#acid_mesh_builtin) |
| `mesh_new` | points_ptr, n_points, faces_ptr, n_faces → i32 | the mesh id, −2 bad mesh, −5 too big; layouts below | [`acid_mesh_new`](09-api-reference.md#acid_mesh_new) |
| `mesh_draw` | id, x, y, size, rx, ry, rz, mode, color | mode 0 wire, 1 solid, 2 both; an unknown id draws nothing | [`acid_mesh_draw`](09-api-reference.md#acid_mesh_draw) |
| `mesh_free` | id | | [`acid_mesh_free`](09-api-reference.md#acid_mesh_free) |
| `window_info` | index, buf, cap → len \| −1 | record `name\tx\ty\tw\th\tfocused` | [`acid_window_info`](09-api-reference.md#acid_window_info) |
| `activate_window` | index | a cart may raise only its own window | [`acid_activate_window`](09-api-reference.md#acid_activate_window) |
| `close_window` | index → i32 (0/1) | **−4 for a cart**, so always −4 for a WASM cart | [`acid_close_window`](09-api-reference.md#acid_close_window) |
| `send_self_to_back` | → | | [`acid_send_self_to_back`](09-api-reference.md#acid_send_self_to_back) |
| `launcher_register` | path_ptr, path_len, name_ptr, name_len, w, h, multi, libs_ptr, libs_len → i32 (0/1) | always 0 for a cart | [`acid_launcher_register`](09-api-reference.md#acid_launcher_register) |
| `launcher_count` | → i32 | | [`acid_launcher_count`](09-api-reference.md#acid_launcher_count) |
| `launcher_path` | index, buf, cap → len \| −1 | | [`acid_launcher_path`](09-api-reference.md#acid_launcher_path) |
| `launcher_name` | index, buf, cap → len \| −1 | | [`acid_launcher_name`](09-api-reference.md#acid_launcher_name) |
| `launcher_spawn` | index → i32 (0/1) | 0 for a cart if that single-instance app is already open or 4 or more cart-level windows are open or its own window is closed; the app runs cart-level; flat 131,072 fuel | [`acid_launcher_spawn`](09-api-reference.md#acid_launcher_spawn) |
| `spawn_app` | path_ptr, path_len, w, h, arg_ptr, arg_len → i32 (0/1) | arg "" for none; a cart only under `v3/apps/`, and 0 for a cart if that single-instance app is already open or 4 or more cart-level windows are open or its own window is closed; the app runs cart-level; flat 131,072 fuel | [`acid_spawn_app`](09-api-reference.md#acid_spawn_app) |
| `composited_frames` | → i32 | saturates at `i32::MAX` | [`acid_composited_frames`](09-api-reference.md#acid_composited_frames) |
| `skipped_frames` | → i32 | saturates at `i32::MAX` | [`acid_skipped_frames`](09-api-reference.md#acid_skipped_frames) |
| `local_time` | buf, cap → len | record `year\tmonth\tday\thour\tmin\tsec` | [`acid_local_time`](09-api-reference.md#acid_local_time) |
| `mem_used_kb` | → i64 | | [`acid_mem_used_kb`](09-api-reference.md#acid_mem_used_kb) |
| `network_info` | buf, cap → len | record `host\tip\tconnected` | [`acid_network_info`](09-api-reference.md#acid_network_info) |
| `refresh_tasks` | → i32 | | [`acid_refresh_tasks`](09-api-reference.md#acid_refresh_tasks) |
| `task_count` | → i32 | | [`acid_task_count`](09-api-reference.md#acid_task_count) |
| `task_info` | index, buf, cap → len \| −1 | record `name\tstate\tcpu` | [`acid_task_info`](09-api-reference.md#acid_task_info) |
| `fs_list` | ptr, len, buf, cap → len \| err | names joined by `\n`; flat 131,072 fuel | [`acid_fs_list`](09-api-reference.md#acid_fs_list) |
| `fs_read` | ptr, len, buf, cap → len \| err | file bytes; the "UTF-8" rule doesn't apply; flat 131,072 fuel | [`acid_fs_read`](09-api-reference.md#acid_fs_read) |
| `fs_size` | ptr, len → i64 size \| err | flat 131,072 fuel | [`acid_fs_size`](09-api-reference.md#acid_fs_size) |
| `fs_write` | ptr, len, data_ptr, data_len → 0 \| err | the data bytes are raw; a cart only under `v3/fsroot/Home/`; flat 131,072 fuel | [`acid_fs_write`](09-api-reference.md#acid_fs_write) |
| `fs_rename` | from_ptr, from_len, to_ptr, to_len → 0 \| err | flat 131,072 fuel | [`acid_fs_rename`](09-api-reference.md#acid_fs_rename) |
| `fs_delete` | ptr, len → 0 \| err | flat 131,072 fuel | [`acid_fs_delete`](09-api-reference.md#acid_fs_delete) |
| `cart_roots` | buf, cap → len \| err | joined by `\n`; −4 for a cart | [`acid_cart_roots`](09-api-reference.md#acid_cart_roots) |
| `cart_list` | ptr, len, buf, cap → len \| err | joined by `\n`; −4 for a cart | [`acid_cart_list`](09-api-reference.md#acid_cart_list) |
| `cart_stat` | ptr, len, buf, cap → len \| err | record `dir\|file\tsize`; −4 for a cart | [`acid_cart_stat`](09-api-reference.md#acid_cart_stat) |
| `cart_read` | ptr, len, buf, cap → len \| err | −4 for a cart | [`acid_cart_read`](09-api-reference.md#acid_cart_read) |

A WASM cart is always cart-level, so `launcher_register`, `overlay_open` and
`close_window` never succeed for it, and the `cart_*` calls always return −4.
They are still in the table so that it matches the Lua calls, and so that the
same ABI can serve other trust levels later.

### Mesh arrays

`mesh_new` reads two arrays of little-endian i32s from the cart's memory:

- **points**: `n_points × 3` values, `x, y, z` for each point. Needs
  `3 ≤ n_points ≤ 512`;
- **faces**: `n_faces × 4` values, four 0-based point indices for each face.
  Put `−1` in the fourth slot for a triangle. The first three slots must be
  in `0 .. n_points`, and the fourth is `−1` or in that range.

A negative count, `n_points < 3`, any bad index, a coordinate beyond ±32767,
or a face that repeats a point gives −2. `n_points > 512`
or `n_faces > 1024` gives −5, before any memory is read. Fewer than 3 points is
checked first, so 2 points with 2000 faces gives −2, where Lua's
`acid_mesh_new` checks the list lengths first and says `"too big"`
([`acid_mesh_new`](09-api-reference.md#acid_mesh_new)). A range outside the
cart's memory traps, like any other pointer. Reading the arrays is charged as
bytes moved (see Fuel).

---

[← API reference](09-api-reference.md) · [Contents](README.md) · [Next: Music →](11-music.md)
