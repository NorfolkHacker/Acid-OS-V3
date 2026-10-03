# 10. WASM carts

[← API reference](09-api-reference.md) · [Contents](README.md)

A **WASM cart** is an app written as a WebAssembly module instead of a Lua
script. The kernel runs it under `wasmi`, an interpreter, in the same kind of
window as a Lua app, and it calls the same operations: drawing, sound, windows,
files. What changes is the shape of the program. A Lua app owns its event loop
and calls `acid_poll_event`; a WASM cart is a set of **callbacks** that the host
calls, and the `acid_*` operations reach it as **imports** from a module named
`"acid"`. This is ABI **version 1**.

This chapter covers the callbacks, how values cross the boundary, the limits,
the header and installing, writing a cart in Rust with the `acid-cart` crate,
and a cart written by hand in the WebAssembly text format. The full import
table is at the end, in [The import table](#the-import-table).

## 10.1 What a WASM cart is

- **It is always cart-level.** The kernel grants built-in trust only to `.lua`
  scripts, so every `.wasm` module runs under the cart rules of
  [§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps) and
  [§7.11](07-system-apis.md#711-what-a-cart-is-refused), however it was started.
  It writes only under `v3/fsroot/Home/`, cannot close or raise other windows,
  and cannot open the overlay.
- **It installs like a `.cart`**, through Load Cart, as `v3/apps/<slug>.wasm`
  with a manifest that says `runtime = wasm`
  ([§2.7](02-apps-and-manifests.md#27-wasm-carts)).
- **It is sandboxed by the module format**, not by a Lua VM: it can touch only
  its own linear memory and the imports in the table. A module that imports
  anything else is refused before it runs.

**When to choose WASM over Lua.** Lua is the default, and the right choice for
almost every app: there is no toolchain, an app is a text file, the libraries
(`AcidApp`, `AcidGame`, `AcidPalette`) do the bookkeeping, and the Editor can
change it in the running OS. Choose WASM when:

- you already have the code in Rust, C or another language that targets
  WebAssembly;
- you want a compiled language's types and tooling;
- you want a cart whose source is not shipped with it.

WASM is not a way to escape the cart rules (it is always cart-level), and under
an interpreter it is not automatically faster than Lua.

## 10.2 The callbacks

A cart **exports** these. The host resolves and type-checks them all before
any callback runs (a start function, if the module has one, has already run),
and refuses the module if a required one is missing or has the wrong
signature.

| Export | Required | Purpose |
|---|---|---|
| `memory` | yes | the cart's linear memory |
| `acid_abi_version() -> i32` | yes | must return `1`, or the cart is refused with a log line |
| `acid_on_create()` | yes | called once, then `acid_redraw` |
| `acid_on_event(kind, a, b, c)` | yes | one input event; the kinds are below |
| `acid_on_idle()` | yes | each poll timeout with no event |
| `acid_redraw()` | yes | on create, and after a "moved" event |
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

Touch coordinates are window-relative and key codes are the ones a Lua app
gets ([§3.1](03-app-lifecycle.md#31-the-callbacks)). A kind the cart does not
know should be ignored: a later ABI version may add some.

**The host owns the loop.** In order, it:

1. instantiates the module (running its start function, if it has one);
2. calls `acid_abi_version`, and refuses the cart unless it returns 1;
3. calls `acid_on_create`, then `acid_redraw`;
4. then, until close: asks `acid_poll_timeout_ms` (if exported), waits that long
   for an event, and calls `acid_on_event` for an event or `acid_on_idle` for a
   timeout.

The poll timeout is clamped to **1–60,000 ms**: values below 1 count as 1, and
anything longer than a minute counts as a minute. A cart never polls, and
nothing is redrawn for it: after a touch that changes what the window shows,
the cart draws it itself (usually by calling its own redraw).

As in Lua, `notify_redraw_done` is a harmless no-op in v3 and the router does
not send "moved" today
([§3.2](03-app-lifecycle.md#moved-and-acid_notify_redraw_done)). `acid-cart`
still calls it after the redraw that follows a move, so a cart written against
it needs no change if the event returns.

## 10.3 Passing values

Every import lives in module `"acid"` and has the name and meaning of the Lua
call without its `acid_` prefix: `fill_rect` is `acid_fill_rect`. The one Lua
call with no import is `acid_poll_event`, because the host owns the loop.
`acid_get_volume` keeps its name as `get_volume`.

**Numbers.** Integers and colours are `i32`; a colour's low 24 bits are read as
`0xRRGGBB`. Booleans are `i32` `0` or `1`. Three results are `i64`: `now_ms`,
`mem_used_kb` and `fs_size`. Window, launcher and task indexes are zero-based,
as in Lua.

**Strings in** are passed as **`(ptr, len)`**: a byte offset into the cart's
memory and a length. If the range is not inside memory, or the bytes are not
valid UTF-8, the import **traps**, and the cart ends. File contents passed to
`fs_write` are raw bytes, so the UTF-8 rule does not apply to them.

**Strings out** are written into a cart-supplied buffer **`(buf, cap)`**. The
call writes `min(len, cap)` bytes at `buf` and **returns the full length**
`len`, so a result longer than `cap` arrives truncated and the cart can retry
with a bigger buffer. A negative return is an error code. The whole
`(buf, cap)` range must be inside memory, even when the result would fit in
less, or the call traps. `fs_read` and `cart_read` write file bytes, which need
not be UTF-8.

**Records.** Calls that answer with several values (`window_info`,
`local_time`, `network_info`, `task_info`, `cart_stat`) write a record:

> A call with several results writes them as one UTF-8 record, its fields
> joined by tab (`\t`) characters, into a cart-supplied `(ptr, cap)` buffer by
> the string-result rule above, and returns the record's full length, or −1
> when there is no such thing (for example, no window at that index). Numbers
> are written in decimal and booleans as `0` or `1`.

For example, `window_info` for the focused window of the sample cart installed
from Load Cart, 200 × 150 at (40, 60), writes
`v3/apps/hello_wasm.wasm\t40\t60\t200\t150\t1` and returns 39. The first field
is the window's script path, not its title. `launcher_path` and
`launcher_name` follow the same rule with a single field: −1 means no app at
that index.

**Lists.** `fs_list`, `cart_roots` and `cart_list` write their names joined by
`\n`, with no trailing newline. An empty directory is an empty string (length
0).

**Error codes.** File and host-cart-folder calls return a negative code where the
Lua call would return `nil, message`:

| Code | Lua message | Meaning |
|---|---|---|
| −1 | `not found` | no such file or directory |
| −2 | `bad path` | the path guard rejected the path ([§7.8](07-system-apis.md#the-path-guard)) |
| −3 | `read only` | a write, rename or delete outside `v3/fsroot/Home/` |
| −4 | `not allowed` | a call carts may not make: the `cart_*` calls, and `close_window` |
| −5 | `too big` | a host cart file over 256 KB |
| −6 | anything else | any other failure (the platform's own message) |

`fs_write`, `fs_rename` and `fs_delete` return 0 on success. `fs_size` returns
the size as an `i64`, or a code.

**Refusals that are not error codes.** The cart rules of
[§7.11](07-system-apis.md#711-what-a-cart-is-refused) apply exactly as in Lua.
Where the Lua call returns `false`, so does the import, as `0`:
`overlay_open` always returns 0, `launcher_register` always returns 0, and
`spawn_app` returns 0 for a path outside `v3/apps/`. `spawn_app` and
`launcher_spawn` also return 0 when the app is single-instance and already
open: a built-in would raise that window, but a cart may not, so nothing is
raised or focused, while 4 or more cart-level windows are open (the
cart's own included), and once the cart's own window has been closed. An app a
cart starts runs cart-level, even a `.lua` under `v3/apps/` that would
otherwise be built-in. A built-in caller never raises a copy of a singleton that a cart started: it opens a trusted window of its own instead. `activate_window` does
nothing unless the index is the cart's own window. The exception is
`close_window`: a cart can never close a window, and it returns **−4**, not 0.
`set_volume` and `set_wallpaper_enabled` work as they do for any app.

## 10.4 Limits

A WASM cart has no Lua memory limit and no "stopped responding" clock. It is held
instead by **fuel** and by **linear-memory pages**.

| Limit | Value |
|---|---|
| Fuel per callback (`WASM_FUEL`) | 200,000,000 |
| Linear memory (`WASM_MAX_PAGES`) | 256 pages of 64 KiB: 16 MB |
| Tables | 1, of at most 65,536 elements |
| Instances, memories | 1 each |
| Module file (Load Cart) | 256 KB |

### Fuel

Every callback starts with a **fresh budget of 200,000,000 fuel**, on the order
of a second of interpreted code. Each WebAssembly instruction burns fuel, and
running out ends the cart. Instantiation is fuelled too: a start function gets
a budget of its own, so an endless loop there ends the cart like one in any
callback.

**Host work is fuel as well**, at **1 fuel per 8 bytes** (rounded up). Without
this, one import call would cost the same whether it moved one byte or 16 MB,
and a cart could keep the host busy far longer than its budget. The host
charges:

- **every byte copied across the boundary**: a string or data argument
  read in, and the whole result of a strings-out call. The whole result is
  charged even when `cap` is smaller, because the host has already produced it,
  so asking for a length with `cap` 0 costs as much as reading it;
- **every draw, by its clipped area**, at 2 bytes per pixel with each side
  clamped to the 640 × 360 screen, before drawing:
  - `fill_rect`, `overlay_fill_rect` and `repaint_region`: `w × h`;
  - `fill_circle`: the bounding square, `(2r + 1)²`;
  - `draw_text`: one 6 × 8 glyph cell per byte, on top of the string's bytes;
  - `draw_window_frame`: a 640 × 16 title bar;
  - `draw_window_border`: the screen's perimeter;
  - `clear_user_area` and `overlay_clear`: the whole screen, **57,600 fuel**;
- **every file-system and spawn call**, a flat **1 MiB-equivalent (131,072
  fuel)** on top of its bytes: `fs_list`, `fs_read`, `fs_size`, `fs_write`,
  `fs_rename`, `fs_delete`, `launcher_spawn` and `spawn_app`. A syscall's real
  cost (a disk round trip, a new task) does not show in its byte count. About
  1,500 such calls fit in one callback, far more than a real cart makes.

If a charge is more than the budget has left, the budget is emptied and the
call ends the cart as out of fuel, exactly as an instruction would.

The budget is per callback, so the way to do a lot of work is the way it is in
Lua: in slices, one per `acid_on_idle`.

### Memory

Linear memory is capped at **256 pages (16 MB)**. A module that declares more
than that ends as out of memory before it runs. Growing past the cap with
`memory.grow` does **not** return −1 to the cart: it **ends the cart as out of
memory**, so a cart cannot ignore a failed allocation and carry on. Tables are
capped the same way (their elements live in host memory outside the 16 MB): one
table of at most 65,536 elements, and growing it past that ends the cart. A
module asking for a second table or memory is refused.

### How a cart ends

In every case the window is removed and the cart's resources are freed. All but
the first print one line to the terminal:

| End | Why | Terminal line |
|---|---|---|
| closed | the close event arrived, and `acid_on_destroy` (if any) returned | none |
| refused | not a valid module; a missing or mistyped export; an import not in the table; `acid_abi_version` not 1; the file could not be read | `Acid OS v3: <path>: refused: <why>` |
| stopped responding | a callback (or the start function) ran out of fuel | `Acid OS v3: <path>: stopped responding` |
| trap | a trap: `unreachable`, a division by zero, a bad string range, a Rust panic | `Acid OS v3: <path>: <trap message>` |
| out of memory | memory or a table declared or grown past its cap | `Acid OS v3: <path>: out of memory` |

A trap or a fuel overrun cannot be caught inside the cart. Other apps are
unaffected: each cart runs on its own thread with its own store.

## 10.5 The header and installing

A `.wasm` cart's header is a **custom section named `acid`**. Its UTF-8 text is
lines of `key: value`, with the keys of a `.cart` header
([§2.5](02-apps-and-manifests.md#the-header)):

```text
name: Hello WASM
w: 200
h: 150
desc: Example WASM cart -- colour-cycling bars
```

`libs` is ignored, since a WASM module loads no Lua. A module with no `acid`
section, or a malformed one, installs under its filename at the default size
(220 × 160) with no description.

To install:

1. Put the `.wasm` in `v3/carts/`, `~/carts` or a removable drive's `carts/`.
2. Open **Load Cart** from the Menu. It lists `.wasm` files beside `.cart` ones
   and reads each header from the custom section.
3. Install it. Load Cart writes `v3/apps/<slug>.app.toml` (with
   `runtime = wasm` before `source = cart`) and then `v3/apps/<slug>.wasm`, the
   manifest first, as for any cart
   ([§2.8](02-apps-and-manifests.md#28-where-an-installed-cart-lands)).
4. Press **RUN**, or open it from the Menu at the next boot.

The sample is `v3/carts/hello_wasm.wasm`.

## 10.6 Writing a cart in Rust with `acid-cart`

The guest crates live in `v3/carts-src/`, a Cargo workspace of their own, so
the OS build never needs the wasm target. `acid-cart` is `no_std`. It gives you:

- **safe wrappers** over every import, at the crate root: `acid_cart::fill_rect`,
  `acid_cart::draw_text(text, x, y, fg, bg)` with a `&str`,
  `acid_cart::fs_read(path, &mut buf)` returning a `Result`, and so on. The raw
  imports are in `acid_cart::sys`;
- the **`Cart` trait**, the **`Event`** and **`Error`** enums;
- the **`acid_cart!`** macro, which defines every export;
- a panic handler, so a panic traps the cart and ends it.

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

Only `new` and `redraw` are required. The host calls `new` and then `on_create`
once (both from `acid_on_create`), then `redraw`, then `on_event` and `on_idle`
until close.

### `Event` and `Error`

```rust cart
pub enum Event {
    Touch { x: i32, y: i32, pressed: bool },
    Key { code: i32, pressed: bool },
    /// The window moved; the host calls `redraw` right after this.
    Moved,
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

`acid_on_event` decodes its four integers into an `Event`, and drops a kind it
does not know. The file and cart-folder wrappers return `Result<_, Error>`.
Indexed records (`window_info`, `launcher_path`, `task_info`) return
`Option<usize>`, `None` for −1. Every buffer call returns the **full** length,
which may be more than the buffer you passed:

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

`acid-cart` has no allocator, and neither does `hello-wasm`: fixed buffers on
the stack or in the cart's struct are the norm.

`acid_cart::close_window` returns `true` only when a window was closed. A
cart is always refused (the import returns −4), so for a cart it is always
`false`.

### `acid_cart!`

Name your type once, at the crate root:

```rust cart
acid_cart!(HelloWasm);
```

It defines `acid_abi_version` (returning `acid_cart::ABI_VERSION`, 1),
`acid_on_create`, `acid_on_event`, `acid_on_idle`, `acid_redraw`,
`acid_poll_timeout_ms` and `acid_on_destroy`. The cart instance lives in a
`static`, created in `acid_on_create` and dropped after `on_destroy`. That is
sound because a wasm32 module without threads has one thread, and the host
calls one export at a time and never re-enters the cart from an import.

### The header

The header is a `static` placed in the `acid` custom section. `#[used]` keeps it
through link-time optimisation, since nothing in the code refers to it. The
array's length must match the text exactly:

```rust cart
#[used]
#[unsafe(link_section = "acid")]
static HEADER: [u8; 78] = *b"name: Hello WASM\nw: 200\nh: 150\ndesc: Example WASM cart -- colour-cycling bars\n";
```

### `hello-wasm`, walked through

`v3/carts-src/hello-wasm` is a port of `v3/apps/hello_acid.lua`: colour-cycling
bars. After the header it keeps its window size in constants that must match
the header's `w` and `h`, as a Lua app's constants match its manifest:

```rust cart
const WINDOW_W: i32 = 200;
const WINDOW_H: i32 = 150;
const TITLE_BAR_H: i32 = 16;
const BAR_H: i32 = 8;
const TEXT_Y: i32 = 70;
```

A `hue(step)` function ports `AcidPalette.hue` with its default 256 steps; it
returns `0xRRGGBB`. The cart's state is one counter:

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

`poll_timeout_ms` asks for an idle every 40 ms. `on_idle` animates only while
the window has focus, as `hello_acid` does, and draws the new frame itself:
nothing redraws a cart for it. `wrapping_add` matters, because an overflowing
`+` would panic, and a panic ends the cart.

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

`redraw` is a Lua `redraw` in Rust: clear, frame, the bars, a label on the bar
it covers, and the border last. Each frame costs about 70,000 fuel of host
work (the clear is 57,600, each bar 400), a tiny fraction of the budget.

## 10.7 Building

You need the `wasm32-unknown-unknown` standard library for the system Rust. On
Arch:

```sh
sudo pacman -S rust-wasm
```

From the repository root, build the example and copy it where Load Cart looks:

```sh
cargo build --manifest-path v3/carts-src/Cargo.toml --release --target wasm32-unknown-unknown -p hello-wasm
cp v3/carts-src/target/wasm32-unknown-unknown/release/hello_wasm.wasm v3/carts/hello_wasm.wasm
```

Then install `hello_wasm.wasm` from Load Cart. The test suite builds the example
from source too, and checks that the fresh module runs.

A cart of your own is a new crate in `v3/carts-src/`: add it to the workspace's
`members`, and give it a `cdylib` target and the `acid-cart` dependency, as
`hello-wasm` does:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
acid-cart = { path = "../acid-cart" }
```

Build it with `-p <crate>`. The output is
`target/wasm32-unknown-unknown/release/<crate>.wasm`, with any `-` in the crate
name turned into `_`. The workspace's release profile (`opt-level = "s"`, LTO,
`panic = "abort"`, stripped) keeps modules small: `hello_wasm.wasm` is about
1 KB, well under the 256 KB cap.

## 10.8 A cart by hand

A cart does not need Rust. Here is a complete one in the WebAssembly text
format. It draws a coloured panel, and a tap flips its colour and plays a note
for as long as you hold it:

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

Every string is a `(ptr, len)` into the cart's own memory: the title is the 8
bytes at offset 0, the label the 6 bytes at 16. The panel's colour lives in a
global, and the touch handler redraws after changing it, because the host will
not. A held touch delivers "pressed" on every router tick, so `$down` makes the
flip happen once per hold, the pattern of
[§3.5](03-app-lifecycle.md#35-touch-debouncing). A note sounds until it is
stopped, so the release stops it.

Turn it into a module with a WAT assembler that understands the `@custom`
annotation, such as the Rust `wat` crate, which the OS's own tests use to
assemble this very example. `wasm-tools parse cart.wat -o cart.wasm` is built
on the same parser and should work too, but it is an optional tool and is not
tested here. An assembler that drops the annotation still
makes a working cart; it just installs under its filename at the default size.
Then install the `.wasm` from Load Cart as above.

## The import table

Every import in module `"acid"`, ABI version 1, in order. All values are `i32`
unless marked `i64`. "len" is a strings-out result: the full length, or a
negative error code where the table says so. The last column is the Lua call it
mirrors, in [chapter 9](09-api-reference.md).

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
They stay in the table so that it mirrors the Lua calls, and so that one ABI
serves any future trust level.

---

[← API reference](09-api-reference.md) · [Contents](README.md)
