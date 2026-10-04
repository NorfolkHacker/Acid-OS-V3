# 4. Graphics

[← The app lifecycle](03-app-lifecycle.md) · [Contents](README.md) · [Next: Sound →](05-sound.md)

Drawing in Acid OS is deliberately simple. You get:

- three shapes: filled rectangles, filled circles and text,
- calls to draw the window's title bar and border,
- a separate full-screen "overlay" for effects that cross the whole screen.

That's the whole graphics API. There are no lines, arcs, rounded rectangles,
transparency or image files. Everything you see, from Tetris pieces to the
rounded window corners to the sprites flying across the screen, is built from
filled rectangles.

Lean into it. The blocky look is on purpose.

The screen is **640×480** pixels by default (640×360 and 800×600 are the other
sizes; ask with [`acid_screen_size`](09-api-reference.md#acid_screen_size)). To
fit every size, keep a window within 640×336 below the strip. Each window has its own canvas, exactly the
size of the `w` and `h` in its manifest. The system stacks those canvases onto
the screen in order, with the top window last. Your drawing calls only ever
touch your own canvas.

## 4.1 Colours

A colour is a plain **24-bit RGB number**, written `0xRRGGBB`.

```lua snippet
acid_fill_rect(10, 20, 50, 30, 0xFF00AA)
```

> Behind the scenes, colours are stored with fewer bits (a format called
> RGB565). Converting just **drops** the lowest bits: the bottom 3 bits of red
> and blue, and the bottom 2 of green. So two colours that differ only in those
> bits look the same. A short value like `0xF800` (the RGB565 way of writing
> red) comes out nearly black. If a colour looks wrong and dark, check that
> you've written all six hex digits.

Coordinates, sizes and radii are all whole numbers. Divide with `//`, as
[chapter 3](03-app-lifecycle.md) does for touch rows, so you never pass a
fraction to a drawing call.

### The theme palette

Six colours give Acid OS its look. They're built into the system, but Lua
**can't read them**, so apps copy the values they need into their own
constants. That's why you'll see the same numbers in lots of apps:

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

For **content**, such as game pieces and pictures, use the 256-colour hue
wheel instead. `AcidPalette` is always loaded, so you never need to list it in
`libs`.

```lua snippet
AcidPalette.hue(step)            -- 256 steps around the wheel
AcidPalette.hue(step, steps)     -- a wheel of `steps` divisions
```

It walks round a full rainbow of bright, fully saturated colours in even
steps, using only whole-number maths. `step` wraps round, and negative steps
wrap too, so you can feed it a counter that keeps growing forever. It returns
a plain `0xRRGGBB` number.

This complete app draws one bar per colour and makes them drift over time. It
sets its frame rate by overriding `poll_timeout_ms`
([§3.3](03-app-lifecycle.md#33-poll_timeout_ms)), and does the animation in
`on_idle`:

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

Why have both? When apps drew their content in theme colours too, everything
ended up looking like the same few shades of green.

## 4.2 Coordinates and clipping

All window drawing is **relative to your window**: `(0, 0)` is its own
top-left corner. Your window is `w × h` pixels, as set in your manifest. The
title bar takes up the top 16 pixels, and the border is 1 pixel wide all the
way round.

### `acid_fill_rect(x, y, w, h, color)`

The call you'll use most. It's **clipped to your window**: any part of a
rectangle that runs off the edge is cut off, and a rectangle that's completely
outside draws nothing. So a wrong coordinate can never paint over the desktop
or another app's window.

```lua snippet
acid_fill_rect(0, 16, WINDOW_W, WINDOW_H - 16, BG_COLOR)  -- fill the body
acid_fill_rect(x, y, 1, h, LINE_COLOR)                    -- a 1px vertical line
acid_fill_rect(x, y, w, 1, LINE_COLOR)                    -- a 1px horizontal line
```

Clipping isn't just a safety net, it can save you work. Piano draws a divider
line at `x = 0` for every white key, including the first one. That first line
just lands harmlessly on the border, so there's no need to treat key 0 as a
special case.

### `acid_fill_circle(x, y, r, color)`

`x, y` is the **centre**, and `r` is the radius.

```lua snippet
acid_fill_circle(ball_x, ball_y, 2, 0x00FF66)
```

Like rectangles, circles are clipped to your canvas. A ball that drifts off
the edge is just cut off, not an error. (A game still needs to keep its own
positions in range so the ball stays in play.) A negative radius draws
nothing.

### `acid_draw_text(str, x, y, fg, bg)`

Draws text in a fixed-width font. Each character is **6×8 pixels**, drawn
with a solid background. Set `bg` to whatever colour is already behind the
text, or you'll get a box around it.

```lua snippet
acid_draw_text("SCORE " .. self.score, 6, 20, TEXT_COLOR, BG_COLOR)
```

`x, y` is the **top-left corner** of the first character, not the baseline.

Text is **clipped to your canvas one character at a time**:

- a character that's partly outside the window is cut off,
- characters wholly off the left edge are skipped,
- drawing stops at the right edge,
- a line of text that's completely above or below the window draws nothing.

That only keeps text inside your *window*, though. A long string will still
run over your border and over anything else you've drawn next to it. Cut
strings to length yourself:

```lua snippet
acid_draw_text(name:sub(1, 22), 6, y, TEXT_COLOR, BG_COLOR)
```

A string of `n` characters is `6 * n` pixels wide and 8 pixels tall. That's
all the maths you need to centre text or work out where to cut it.

If `fg` and `bg` are the **same colour**, the background isn't painted at all.
Only the character's own pixels are drawn, in `fg`. That's the one way to put
text over a picture without a box behind it.

The font covers character codes up to 255. Anything above that shows as an
outlined box. Stick to ASCII.

### Text size

`acid_draw_text` draws at your window's scale: 1 (the 6×8 above), or 2 (12×16)
when your app opted in with `font = scalable` and Config's FONT setting was
Large when the app opened. Don't lay text out from 6 and 8. Read the character
size and window size once, in `on_create`, and work from those:

```lua snippet
local CW, CH = acid_font_size()
local cols = (acid_window_size() - 8) // CW
```

## 4.3 Window chrome

"Chrome" means the parts of the window that belong to the system: the title
bar, the close button and the border. Three calls draw them.

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

You draw your content anywhere from `(0, 0)` to the full width and height of
the window, and that includes the pixels where the border goes. If you draw
the border first, your content paints right over it. That's why the border is
its own call, separate from `acid_draw_window_frame`.

The rounded corners are a 3-pixel staircase, the trick old low-resolution
screens used before smooth curves were practical. They have to be cut *after*
the straight border lines are drawn, and `acid_draw_window_border` does that
for you. On a window smaller than 6 pixels in either direction, the corner
size shrinks to half the shorter side.

### Titles

`acid_draw_window_frame` does **not** stop the title at the close button.
Keep titles short. That's exactly why `AcidApp:window_title` cuts them to 16
characters.

## 4.4 Partial redraws and flicker

Clearing with `acid_clear_user_area` and then repainting everything is simple
and correct. It's what you want for the first paint.

It's **not** what you want on every key press or button press, though.
Clearing the whole window to blank and then redrawing it causes a visible
flash.

The fix is to repaint only what changed. `piano.lua` is a good example to
copy:

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

When you repaint just one area, watch out for two things:

- **Overlap.** If something else is drawn on top of that area, repainting the
  area alone wipes it out. Piano repaints a white key *and* any black keys
  that overlap its edges.
- **The border.** If the area touches the edge of the window, call
  `acid_draw_window_border` again afterwards, or stop your repaint just short
  of the edge.

There's a third trick for apps that redraw on a timer: skip the repaint when
nothing has changed. Keep a **state signature**, a single value that captures
everything that affects the picture, and only redraw when it's different.
Lua compares tables by identity, not by contents, so build the signature as a
string:

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

The overlay is **a single screen-sized canvas that belongs to the system**. It
is drawn on top of everything else, and magenta (`0xFF00FF`) counts as
see-through. It's the only way to draw across the *whole* screen: over the
wallpaper, the desktop strip and every open window, your own included.

It isn't a window:

- it doesn't belong to any app,
- clicks pass straight through it to whatever is underneath,
- it never takes focus,
- it never shows up in the taskbar.

```lua snippet
acid_overlay_open()        -- claim it; true on success, false if refused
acid_overlay_clear()       -- fill the whole screen with the transparency key
acid_overlay_fill_rect(x, y, w, h, color)   -- screen-absolute coordinates
acid_overlay_close()       -- release the claim
```

The see-through check happens after the colour has been stored in RGB565
([§4.1](#41-colours)). So any colour that ends up the same as `0xFF00FF` after
the low bits are dropped is see-through too. Keep your real colours well away
from magenta.

> **Carts can't open the overlay.** A cart-level app (an installed `.cart` or
> a WASM cart, [§2.5](02-apps-and-manifests.md#25-carts)) never gets to take
> over the whole screen. For a cart, `acid_overlay_open` always returns
> `false`, and because a cart can never own the overlay, its other overlay
> calls do nothing. Write the `false` branch anyway, because a built-in app
> can get `false` too, as the next section explains.

### One owner at a time

`acid_overlay_open` returns `false` if another app already has the overlay
(or if you're a cart). There's only one overlay. If a second animation started
halfway through the first, each would keep clearing the other's frames and
they'd fight.

**Being refused isn't an error.** If an effect can't start because another one
is already running, it should just do nothing:

```lua snippet
function MyApp:start_effect()
  if not acid_overlay_open() then return end
  self.frame = 0
  self.running_effect = true
end
```

Note the name `running_effect`, not `running`: `self.running` is `AcidApp`'s
loop flag. See the note at the top of [chapter 3](03-app-lifecycle.md).

If you already own the overlay, opening it again works and clears it. That's
handy for restarting your own animation.

When your app ends, its claim on the overlay is released automatically,
however it ended. `acid_overlay_close` does nothing unless you're the current
owner, so you can never close someone else's animation.

### Coordinates are screen-absolute

`acid_overlay_fill_rect` takes coordinates on the screen, not in your
window. It clips to the screen edges, so negative coordinates are fine. That's
how a sprite flies in from off-screen.

If you draw while you don't own the overlay, nothing happens, silently. That's
deliberate. A sprite whose animation ended a frame ago, or an effect that never
got the overlay, shouldn't cause an error anyone sees.

### An animation loop

This complete app flies a bar across the whole screen, then gives the overlay
back. It animates from `on_idle`, with a short `poll_timeout_ms` while the
effect is running and a long one the rest of the time:

```lua app
local FlyApp = AcidApp:extend("FlyApp")
local SCREEN_W = acid_screen_size()

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
  if self.x > SCREEN_W then
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

`on_touch` has the guard against held presses that
[§3.5](03-app-lifecycle.md#35-touch-debouncing) asks for, so tapping again
while the effect is running does nothing.

Each frame clears the overlay and then draws, so for a brief moment the
overlay is empty. If the screen happens to update right then, the worst that
happens is one frame without the bar. That moment lasts tens of microseconds,
and the screen updates about every 16 ms. Every window in Acid OS draws this
way already.

## 4.6 Sprites

`AcidSprite` draws small pictures onto the overlay. It lives in
`v3/apps/lib/acid_sprite.lua`, so add that to your manifest's `libs` to use
it.

You write a sprite **as a picture**: a list of strings, all the same length,
with one character per pixel, plus a palette table that maps characters to
colours. `.` is see-through, and so is any character that isn't in the
palette.

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

To change a sprite, you **redraw it in the source**. No coordinates to work
out.

- `scale` turns each character into a `scale × scale` block, so a 7×3 drawing
  at `scale = 4` is 28×12 pixels on screen.
- `x, y` are screen coordinates, because sprites draw on the overlay.
- You must own the overlay. `AcidSprite` draws with `acid_overlay_fill_rect`,
  which does nothing if you don't.
- The last argument, `flip`, mirrors the sprite left to right. One drawing can
  then face either way, so a sprite flying right-to-left looks like it's going
  forwards, not backwards.

Neighbouring pixels of the same colour are drawn as one rectangle, so a sprite
takes far fewer drawing calls than it has pixels.

This complete app flies a sprite across the screen and flips it round each
time. Its first line, `-- libs: lib/acid_sprite.lua`, tells the test suite to
load the sprite library. A real installed app would put the same path in its
manifest's `libs` instead ([§2.3](02-apps-and-manifests.md#23-loading-modules)):

```lua app
-- libs: lib/acid_sprite.lua
local SpriteApp = AcidApp:extend("SpriteApp")
local SCREEN_W = acid_screen_size()

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
  if self.dir == 1 then self.x = -w else self.x = SCREEN_W end
end

function SpriteApp:on_idle()
  if not self.flying then return end
  local w = AcidSprite.width(SHIP) * SCALE
  self.x = self.x + 6 * self.dir
  if self.x > SCREEN_W or self.x < -w then
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

This is a system-wide setting, and the Config app is in charge of it. Turning
it on or off redraws the screen straight away. With the wallpaper off, the
desktop is plain `THEME_BG`. Like `acid_set_volume`, carts are allowed to use
it.

`acid_repaint_region(x, y, w, h)` fills that area **of your own canvas** with
the wallpaper, or with `THEME_BG` if the wallpaper is off. Despite its name,
it doesn't ask anything else to repaint. The screen is rebuilt from every
window's canvas whenever anything changes, so clearing your own canvas is
enough: next time the screen updates, whatever is really underneath shows
through.

It copies the wallpaper from *the same coordinates* as the area you give it.
So it only looks right for a window that sits at the top-left of the screen,
`(0, 0)`. In practice only the desktop strip uses it, to clear away a
dropdown menu when it closes.

---

[← The app lifecycle](03-app-lifecycle.md) · [Contents](README.md) · [Next: Sound →](05-sound.md)
