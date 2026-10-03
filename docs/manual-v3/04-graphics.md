# 4. Graphics

[← The app lifecycle](03-app-lifecycle.md) · [Contents](README.md) · [Next: Sound →](05-sound.md)

There are three drawing primitives (rectangle, circle, text) plus the window
chrome and a separate full-screen overlay. That is the whole graphics API. There
is no line, no arc, no rounded rectangle, no alpha, no image loading. Everything
in this OS, from Tetris pieces to the rounded window corners to the sprites that
fly across the screen, is built from filled rectangles.

Lean into it. The look is deliberate.

The screen is **640×360**. Every window has a canvas of its own, exactly as big
as the `w` and `h` in its manifest, and the kernel composites those canvases onto
the screen in z-order. Your drawing calls only ever touch your own canvas.

## 4.1 Colours

Colours are plain **24-bit RGB integers**: `0xRRGGBB`.

```lua snippet
acid_fill_rect(10, 20, 50, 30, 0xFF00AA)
```

> The canvas stores colour as RGB565 internally, and a 24-bit value is
> converted by **truncation**: the low 3 bits of red and blue and the low 2 of
> green are dropped. Two colours that differ only in those bits come out the
> same. Early bring-up code used RGB565-style literals like `0xF800` and they
> rendered as near-black. If a colour comes out wrong and dark, check that it
> is a full 24-bit value.

All coordinates, sizes and radii are integers. Divide with `//`, as
[chapter 3](03-app-lifecycle.md) does for touch rows, so you never hand a
fraction to a drawing call.

### The theme palette

Six colours define the OS's identity. They are Rust constants in the kernel
(`acid-kernel`'s `theme.rs`) and are **not** exposed to Lua, so apps declare the
ones they use as their own constants, which is why you see the same literals
across the tree:

| Constant | Value | Used for |
|---|---|---|
| `THEME_BG` | `0x050607` | Window body background, desktop |
| `THEME_HARD` | `0x00FF66` | Acid green: borders, accents, pressed states |
| `THEME_PANEL` | `0x0B1712` | Title bars, panels |
| `THEME_TEXT` | `0xD4E6DB` | Body text |
| `THEME_MUTED` | `0x9DAAA3` | Secondary text |
| `THEME_VIOLET` | `0xB026FF` | Launchable/executable accent |

```lua snippet
local BG_COLOR = 0x050607      -- THEME_BG
local TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
local ACCENT_COLOR = 0x00FF66  -- THEME_HARD
```

Use the theme for **UI**: anything that should look like part of the system.

### `AcidPalette`: the hue wheel

For **content**, use the 256-colour hue wheel. `AcidPalette` is always loaded;
you never list it in `libs`.

```lua snippet
AcidPalette.hue(step)            -- 256 steps around the wheel
AcidPalette.hue(step, steps)     -- a wheel of `steps` divisions
```

A full HSV hue wheel at maximum saturation and value, walked in even increments:
a real continuous rainbow, integer-only, no `math` needed. `step` wraps (a
negative step wraps too), so you can feed it a counter that grows forever. It
returns a plain `0xRRGGBB` integer.

This complete app draws one bar per hue and cycles them over time. Note that
`poll_timeout_ms` is overridden to set the frame rate ([§3.3](03-app-lifecycle.md#33-poll_timeout_ms)),
and that the animation sits in `on_idle`:

```lua app
-- w: 200
-- h: 120
local HueApp = AcidApp:extend("HueApp")

local TITLE_BAR_H = 16
local BARS = 8
local BAR_H = 12

function HueApp:on_create()
  self.step = 0
end

function HueApp:poll_timeout_ms()
  return 50
end

function HueApp:on_idle()
  if not self:focused() then return end
  self.step = self.step + 3
  self:redraw()
end

function HueApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  for row = 0, BARS - 1 do
    -- a distinct, vivid colour per row, drifting over time
    local colour = AcidPalette.hue(self.step + row * 20)
    acid_fill_rect(8, TITLE_BAR_H + 6 + row * BAR_H, 184, BAR_H - 2, colour)
  end
  acid_draw_window_border()
end

HueApp:new():start()
```

This exists because apps used to draw *content* from the five theme colours too,
and everything ended up looking like the same few shades of green.

## 4.2 Coordinates and clipping

All window drawing is in **window-relative** coordinates. `(0, 0)` is your
window's own top-left corner. Your window is `w × h` as declared in your
manifest; the title bar is the top 16 pixels and the border is 1 pixel all
round.

### `acid_fill_rect(x, y, w, h, color)`

The workhorse. **Clipped against your window's bounds**: a rectangle that runs
off an edge is trimmed, and one entirely outside draws nothing. You cannot paint
onto the desktop or another app's window by miscalculating a coordinate.

```lua snippet
acid_fill_rect(0, 16, WINDOW_W, WINDOW_H - 16, BG_COLOR)  -- fill the body
acid_fill_rect(x, y, 1, h, LINE_COLOR)                    -- a 1px vertical line
acid_fill_rect(x, y, w, 1, LINE_COLOR)                    -- a 1px horizontal line
```

Clipping is genuinely useful, not just a safety net. Piano draws a divider line
at `x = 0` for every white key and lets the first one clip harmlessly onto the
border, instead of special-casing index 0.

### `acid_fill_circle(x, y, r, color)`

`x, y` is the **centre**, `r` the radius.

```lua snippet
acid_fill_circle(ball_x, ball_y, 2, 0x00FF66)
```

Like the rectangle it is clipped to your canvas, so a ball that drifts off an
edge is trimmed, not an error. (A game still wants to clamp its own positions
so the ball stays in play.) A negative radius draws nothing.

### `acid_draw_text(str, x, y, fg, bg)`

Fixed-width bitmap font, **6×8 pixels per glyph**, drawn with an opaque background.
`bg` must be the colour that is already behind the text, or you get a box.

```lua snippet
acid_draw_text("SCORE " .. self.score, 6, 20, TEXT_COLOR, BG_COLOR)
```

`x, y` is the **top-left** of the first glyph, not a baseline.

Text is **clipped to your canvas glyph by glyph**: a glyph that is partly outside
the window is trimmed, glyphs wholly off the left edge are skipped, drawing stops
at the right edge, and a string whose row lies wholly above or below the window
draws nothing. Clipping to the *window* is all it does, though. A long string
still runs across your own border and over whatever you drew beside it, so cut
strings to length yourself:

```lua snippet
acid_draw_text(name:sub(1, 22), 6, y, TEXT_COLOR, BG_COLOR)
```

A string of `n` characters is `6 * n` pixels wide and 8 tall: the arithmetic
you need for centring and for deciding where to truncate.

If `fg` and `bg` are the **same colour**, the glyph cell's background is not
painted at all: only the lit pixels are drawn, in `fg`. That is the one way to
draw text over a picture without a box behind it.

The font covers character codes up to 255; a character past that draws as an
outlined box. Stick to ASCII.

## 4.3 Window chrome

Three calls draw the parts of the window that belong to the system.

```lua snippet
function MyApp:redraw()
  acid_clear_user_area()                      -- 1. clear the body to THEME_BG
  acid_draw_window_frame(self:window_title())  -- 2. title bar, title, close button
  -- ... your content ...                      -- 3. everything you draw
  acid_draw_window_border()                    -- 4. outline and rounded corners
end
```

| Call | Does |
|---|---|
| `acid_clear_user_area()` | Fills everything below the title bar with `THEME_BG`. Does not touch the title bar. |
| `acid_draw_window_frame(title)` | Fills the 16px title bar with `THEME_PANEL`, draws `title` at `(4, 4)`, and the green close dot (radius 5, centred 8 pixels in from the right edge, at `y = 8`). |
| `acid_draw_window_border()` | 1px `THEME_HARD` outline around the whole window, then cuts the four rounded corners. |

### The border must come last

Your content is drawn in coordinates running from `(0, 0)` to the window's full
width and height, which is exactly where the border's own pixels sit. Draw the
border first and your content paints straight over it. This is why it is a
separate call rather than part of `acid_draw_window_frame`.

The rounded corners are a 3-pixel staircase, the technique classic low-resolution
GUIs used before anti-aliased curves were affordable. They must be cut *after*
the straight border lines, which `acid_draw_window_border` handles internally.
(On a window under 6 pixels in either direction the radius shrinks to half the
smaller side.)

### Titles

`acid_draw_window_frame` does **not** clip the title against the close button.
Keep titles short: `AcidApp:window_title` caps at 16 characters for exactly
this reason.

## 4.4 Partial redraws and flicker

`acid_clear_user_area` + full repaint is correct and simple, and it is what you
want for the initial paint. It is **not** what you want on every keystroke or
button press: clearing the whole body to blank and redrawing it flashes visibly.

The fix is to repaint only what changed. `piano.lua` is the model:

```lua snippet
-- Full repaint: the initial paint.
function Piano:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_white_keys()
  self:draw_black_keys()
  acid_draw_window_border()
end

-- Per-press: repaint only the one key whose state changed.
function Piano:redraw_offset(offset)
  -- ...redraw that key, plus anything drawn on top of it...
end
```

Two things to watch when repainting a region:

- **Overlap.** If something is drawn on top of your region, repainting the
  region alone erases it. Piano repaints a white key *and* any black keys that
  straddle its edges.
- **The border.** If your region touches the window edge, call
  `acid_draw_window_border` again afterwards, or redraw a stripe shy of the edge.

A third technique, for apps that redraw on a timer: keep a **state signature**:
a plain comparable value of everything that affects the picture, and skip the
repaint entirely when it has not changed. Lua tables compare by identity, so
build the signature as a string:

```lua snippet
function MyApp:state_signature()
  return table.concat({ self.score, self.lives, self.level, self.selected }, ",")
end

function MyApp:redraw_if_changed()
  local sig = self:state_signature()
  if sig == self.drawn_sig then return end
  self.drawn_sig = sig
  self:redraw()
end
```

## 4.5 The overlay

The overlay is **one screen-sized canvas owned by the kernel**, composited last,
with magenta `0xFF00FF` treated as transparent. It is the only way to draw
across the *whole* screen: over the wallpaper, the desktop strip and every open
window, including your own.

It is not a window. It owns no task, is never hit-tested (clicks land on whatever
is really underneath), never takes focus, and never appears in the taskbar.

```lua snippet
acid_overlay_open()        -- claim it; true on success, false if refused
acid_overlay_clear()       -- fill the whole screen with the transparency key
acid_overlay_fill_rect(x, y, w, h, color)   -- screen-absolute coordinates
acid_overlay_close()       -- release the claim
```

The transparency test is made on the stored RGB565 value, so any colour that
truncates to the same value as `0xFF00FF` is transparent too. Keep real colours
well away from magenta.

> **Carts cannot open the overlay.** A cart-level app (an installed `.cart` or
> a WASM cart, [§2.5](02-apps-and-manifests.md#25-carts)) never takes the whole
> screen: for a cart `acid_overlay_open` always returns `false`, and since a
> cart can never own the overlay, its other overlay calls do nothing. Write the
> `false` branch anyway. A built-in app can see it too, as below.

### One owner at a time

`acid_overlay_open` returns `false` if another task already holds the overlay
(or if you are a cart). There is exactly one canvas, so a second animation
starting mid-flight would clear the first one's frames and the two would fight.

**A refusal is not an error.** An effect that declines to start because another
one is already running should simply do nothing:

```lua snippet
function MyApp:start_effect()
  if not acid_overlay_open() then return end
  self.frame = 0
  self.running_effect = true
end
```

(Not `self.running`: that name is `AcidApp`'s loop flag. See the note at the top
of [chapter 3](03-app-lifecycle.md).)

Re-opening from the task that already owns it succeeds and re-clears, which is
handy for restarting your own animation.

The claim is released automatically when your app ends, on every exit path.
`acid_overlay_close` does nothing unless you are the current owner, so you can
never close someone else's animation.

### Coordinates are screen-absolute

`acid_overlay_fill_rect` takes screen coordinates on a 640×360 screen, not
window coordinates. It clips against the screen, so negative coordinates are
normal: that is how a sprite flies in from off-screen.

Drawing while you do not own the overlay silently does nothing. A sprite whose
animation ended a frame ago, or an effect that never got a canvas, must not be
an error anyone sees.

### An animation loop

This complete app flies a bar across the whole screen, then releases the
overlay. It animates from `on_idle` with a short `poll_timeout_ms` while the
effect runs and a slow one otherwise:

```lua app
local FlyApp = AcidApp:extend("FlyApp")

local FRAME_MS = 33

function FlyApp:on_create()
  self.effect_running = false
  self.x = -40
end

function FlyApp:poll_timeout_ms()
  return self.effect_running and FRAME_MS or 200
end

function FlyApp:on_touch(x, y, pressed)
  if not pressed or self.effect_running then return end
  if acid_overlay_open() then
    self.x = -40
    self.effect_running = true
  end
end

function FlyApp:on_idle()
  if not self.effect_running then return end
  self.x = self.x + 6
  if self.x > 640 then
    acid_overlay_close()
    self.effect_running = false
    return
  end
  acid_overlay_clear()
  acid_overlay_fill_rect(self.x, 120, 40, 12, AcidPalette.hue(self.x // 3))
end

function FlyApp:on_destroy()
  acid_overlay_close()
end

FlyApp:new():start()
```

(`on_touch` has the held-press guard [§3.5](03-app-lifecycle.md#35-touch-debouncing)
asks for: a second tap while the effect runs is ignored.)

Clear-then-draw means there is a brief window where the canvas is all-key with
nothing drawn yet. If a composite lands inside it, the worst case is one frame
with the bar missing, tens of microseconds against a ~16 ms compositor tick.
Every window canvas in this OS already draws this way.

## 4.6 Sprites

`AcidSprite` (`v3/apps/lib/acid_sprite.lua`; add it to your manifest's `libs`)
draws pictures onto the overlay. A sprite is written **as a picture**: a Lua
sequence of equal-length strings, one character per pixel, plus a palette table.
`.` is transparent. A character with no palette entry is transparent too.

```lua snippet
local TEAPOT = { "..WWW..",
                 ".WWWWW.",
                 "WWWWWWW" }
local PALETTE = { W = 0xE8E8F0 }

AcidSprite.draw(TEAPOT, x, y, scale, PALETTE)
AcidSprite.draw(TEAPOT, x, y, scale, PALETTE, true)   -- mirrored horizontally
AcidSprite.width(TEAPOT)    -- in characters, not pixels: multiply by scale
AcidSprite.height(TEAPOT)
```

You edit a sprite by **redrawing it in the source**, not by recomputing
coordinates. `scale` multiplies each character into a `scale × scale` block, so
a 7×3 drawing at `scale = 4` is 28×12 on screen. `x, y` are screen coordinates,
the overlay's, and the caller must own the overlay: `AcidSprite` draws with
`acid_overlay_fill_rect`, which does nothing otherwise.

`flip` mirrors horizontally, so one drawing of a character can face either way,
which is what makes a sprite flying right-to-left look like it is facing where
it is going rather than flying backwards.

Runs of identical colour are merged into single rectangles, so a sprite costs
far fewer drawing calls than it has pixels.

This complete app flies a sprite across the screen, mirroring it each pass. Its
header declares `-- libs: lib/acid_sprite.lua` for the test harness; an
installed app would put the same path in its manifest's `libs`
([§2.3](02-apps-and-manifests.md#23-loading-modules)):

```lua app
-- libs: lib/acid_sprite.lua
local SpriteApp = AcidApp:extend("SpriteApp")

local FRAME_MS = 33
local SCALE = 4
local SHIP = {
  "...GG...",
  "..GGGG..",
  ".GGWWGG.",
  "GGGWWGGG",
  "GG.GG.GG",
  "R......R",
}
local PALETTE = { G = 0x00FF66, W = 0xD4E6DB, R = 0xB026FF }

function SpriteApp:on_create()
  self.flying = false
  self.dir = 1
  self.x = 0
end

function SpriteApp:poll_timeout_ms()
  return self.flying and FRAME_MS or 200
end

function SpriteApp:on_touch(x, y, pressed)
  if not pressed or self.flying then return end
  if not acid_overlay_open() then return end
  self.flying = true
  local w = AcidSprite.width(SHIP) * SCALE
  if self.dir == 1 then self.x = -w else self.x = 640 end
end

function SpriteApp:on_idle()
  if not self.flying then return end
  local w = AcidSprite.width(SHIP) * SCALE
  self.x = self.x + 6 * self.dir
  if self.x > 640 or self.x < -w then
    acid_overlay_close()
    self.flying = false
    self.dir = -self.dir
    return
  end
  acid_overlay_clear()
  AcidSprite.draw(SHIP, self.x, 120, SCALE, PALETTE, self.dir == -1)
end

function SpriteApp:on_destroy()
  acid_overlay_close()
end

SpriteApp:new():start()
```

## 4.7 The wallpaper

```lua snippet
acid_set_wallpaper_enabled(true)   -- or false
acid_get_wallpaper_enabled()       -- => true / false
```

A system-wide setting; Config owns it. Toggling it flips the flag and asks the
compositor to recomposite; with it off the desktop is plain `THEME_BG`. Like
`acid_set_volume`, it stays open to carts.

`acid_repaint_region(x, y, w, h)` fills that region **of your own canvas** with
the wallpaper (or `THEME_BG` when the wallpaper is off). Despite the name it does
not ask anyone else to repaint: the screen is recomputed from each window's
canvas every time anything changes, so erasing your own claim on a region is all
that is needed, and the next composite shows what is really underneath.

The pixels it copies are the wallpaper's *at the same coordinates as the
region*, so it is only correct for a window that sits at the screen's `(0, 0)`.
The desktop strip's dropdown-close is the one caller.

---

[← The app lifecycle](03-app-lifecycle.md) · [Contents](README.md) · [Next: Sound →](05-sound.md)
