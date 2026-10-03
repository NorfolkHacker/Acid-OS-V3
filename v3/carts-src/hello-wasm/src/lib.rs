//! Hello WASM: the example WASM cart, a port of `v3/apps/hello_acid.lua`
//! (colour-cycling bars). Built and committed as `v3/carts/hello_wasm.wasm`
//! (spec §15.5); Load Cart installs it like a `.cart`.

#![no_std]

use acid_cart::{Cart, acid_cart};

/// The cart header (spec §15.4): a custom section named `acid` holding the
/// same `key: value` lines as a `.cart` header. Load Cart reads it with
/// `Cartfile.wasm_header`. `#[used]` keeps it through LTO, since nothing
/// in the code refers to it.
#[used]
#[unsafe(link_section = "acid")]
static HEADER: [u8; 78] = *b"name: Hello WASM\nw: 200\nh: 150\ndesc: Example WASM cart -- colour-cycling bars\n";

// Must match the header's w/h, as hello_acid's constants match its manifest.
const WINDOW_W: i32 = 200;
const WINDOW_H: i32 = 150;
const TITLE_BAR_H: i32 = 16;
const BAR_H: i32 = 8;
const TEXT_Y: i32 = 70;

/// AcidPalette.hue (`v3/apps/lib/acid_palette.lua`) with its default 256
/// steps: a vivid hue wheel. `rem_euclid` floors like Lua's `%`, so a
/// negative step wraps the same way; after it every term is non-negative,
/// so `/` matches Lua's `//`.
fn hue(step: i32) -> u32 {
    const STEPS: i32 = 256;
    let h = step.rem_euclid(STEPS) * 360 / STEPS;
    let sector = h / 60;
    let f = h % 60;
    let rise = (255 * f / 60) as u32;
    let fall = 255 - rise;
    let (r, g, b) = match sector {
        0 => (255, rise, 0),
        1 => (fall, 255, 0),
        2 => (0, 255, rise),
        3 => (0, fall, 255),
        4 => (rise, 0, 255),
        _ => (255, 0, fall),
    };
    (r << 16) | (g << 8) | b
}

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
