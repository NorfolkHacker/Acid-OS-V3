# Sprite Paint — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

v1 RaveOS had PAINT, a 16×16 pixel editor. Neither v2 nor v3 has an
equivalent. Sprite Paint brings one back and improves on it:

- several sizes and an editable palette;
- animation frames;
- a text sprite format that games and carts load through the shared
  `AcidSprite` library, so what you draw can be used in apps.

## Decisions

| Question | Choice |
|---|---|
| Scope | One app for pixel art and sprites. The v1 Forth console is not ported, because Lua and Terminal already cover scripting. |
| Format | A text `.spr` file, described in §1. |
| Features | Sizes 8, 16 and 32. An editable 16-colour palette. Pencil, Fill, Eraser, Picker and Line tools. Mirror drawing. Undo and redo. A live preview. Animation frames with playback. |
| Architecture | A Lua app (`apps/sprite.lua` and `apps/sprite/`), with the format code in `apps/lib/acid_sprite.lua`. There are no kernel, API or wasm changes. |

## Out of scope

- Free-form canvases larger than 32×32, brushes, and shapes other than
  lines.
- Layers, selections, and copy/paste of regions.
- Exporting to PNG or other image formats.
- A file-open dialog inside the app. Files are opened from File Manager.
- Loading `.spr` files from wasm carts. Carts can parse the text
  themselves, and a guest-crate helper can come later.

## 1. The `.spr` format and library

### 1.1 Format

```
acid-sprite 1
size 16 16
fps 6
pal 0 000000
pal 1 ff00ff
frame
................
..11............
...
frame
...
```

- **Header:** the first non-blank, non-comment line must be exactly
  `acid-sprite 1`.
- **Ignored lines:** blank lines and lines whose first character is `#`.
  A trailing `\r` is stripped from every line.
- **`size W H`:** required, and must come before the first `frame`. W and H
  are integers from 1 to 32.
- **`fps N`:** optional, an integer from 1 to 30. The default is 6.
- **`pal K RRGGBB`:**
  - K is one character from `0-9a-f`;
  - the colour is exactly 6 hex digits, in either case;
  - there are at most 16 entries, and a key may not repeat;
  - every `pal` line must come before the first `frame`.
- **`frame`:** starts a frame, and must be followed by exactly H rows.
  - Each row has exactly W characters.
  - Each character is `.` (transparent) or a declared palette key.
  - A file has 1 to 8 frames.
- **Errors:** any other line, a missing `size`, or a violation of the rules
  above is an error.

### 1.2 Library (`apps/lib/acid_sprite.lua`)

These functions are added next to the existing `width`, `height`, `draw`
and `draw_row`, which stay unchanged.

- **`AcidSprite.parse(text)`**
  - On success it returns
    `{ w = W, h = H, fps = N, palette = { ["1"] = 0xff00ff, ... }, frames = { rows, ... } }`.
    Each `rows` is a list of H strings, which is the picture-string form
    `AcidSprite.draw` already takes. A game can therefore call
    `AcidSprite.draw(s.frames[1], x, y, scale, s.palette)` directly.
  - On failure it returns `nil, "line N: reason"`, where N is the 1-based
    line number in the text. It never raises.
- **`AcidSprite.serialize(s)`** returns the text in a fixed order:
  1. the header;
  2. `size`;
  3. `fps`;
  4. the `pal` lines, sorted by key, with lowercase hex;
  5. each frame.

  It writes no comments, no blank lines, and a final newline.
  `serialize(parse(t))` is a fixed point: serialising a parsed result and
  parsing it again gives an equal table, and serialising that gives the
  same text.
- **`AcidSprite.load(path)`** calls `acid_fs_read(path)`, then `parse`. A
  read error is returned as `nil, msg`.

Sprite files are saved under `v3/fsroot/Home`.

## 2. The app: Sprite Paint

### 2.1 Launching

- **Manifest** `apps/sprite.app.toml`:
  - `name = Sprite Paint`;
  - `w = 360`, `h = 260`;
  - `multi = true`;
  - `resizable = true`, `min_w = 280`, `min_h = 200`;
  - `libs` lists `lib/acid_sprite.lua`, `lib/acid_palette.lua` and the
    `sprite/` modules.

  It shows in the Menu.
- **From the Menu**, with an empty launch argument, it opens a new,
  untitled 16×16 sprite with one frame, the default palette (§2.5), and
  colour `1` selected.
- **With a launch argument** (a path), it loads that file. If the load
  fails, it opens a new sprite and shows the error on the message line.
- **File Manager** opens a `.spr` row by spawning Sprite Paint at 360×260
  with the file's path. This is the same pattern `.lua` files use for
  Editor.

### 2.2 Layout (`sprite/layout.lua`)

`SpriteLayout.compute(w, h, sprite_w, sprite_h)` recomputes every
rectangle. It runs from `on_create`, from `on_resize`, and after `new` or
`load` changes the sprite size.

- **Right column:** 100 px wide, 4 px inside the right border.
  - Five tool buttons and the Mirror toggle, in two rows of three.
  - The 4×4 palette swatches.
  - The preview, which draws the current frame at 1× and at 2×.
- **Bottom bar:** one text row above the bottom border, holding the frame
  bar `< 2/4 >  +  dup  del  play  6fps`.
- **Message line:** above the frame bar, showing `new` (never saved, no
  changes), `saved`, `unsaved`, or the last message (an error, `saved`). It
  ends with the file name, or `untitled`. While a prompt is open it shows the
  prompt instead.
- **Canvas:** the rest of the window. Each cell is
  `cell = max(1, min(avail_w // sprite_w, avail_h // sprite_h))` px, and the
  canvas is centred in its area.
  - At the default window a 16×16 sprite gets 13 px cells and a 32×32
    sprite 6 px; at the minimum window a 32×32 sprite gets 4 px.
  - Transparent cells draw as a two-tone checker.
  - When `cell >= 4`, a 1 px grid line in a muted colour separates the
    cells.

### 2.3 Document (`sprite/doc.lua`)

`SpriteDoc` holds the sprite in the same table shape `AcidSprite.parse`
returns. It has no drawing and no file I/O. Its operations:

- `get(x, y)` and `set(x, y, key)`, on the current frame. Out-of-range
  coordinates are ignored.
- `frame_index`, `frame_next()`, `frame_prev()`. These wrap around.
- `frame_add()` adds a blank frame after the current one. `frame_dup()`
  copies the current frame after it. Both refuse at 8 frames.
- `frame_del()` deletes the current frame, and refuses if it is the last
  one.
- `set_color(key, rgb)` and `set_fps(n)`, clamped to 1–30.
- Undo and redo:
  - `begin_step()` and `end_step()` bracket each undoable action. An action
    that changed nothing records no step.
  - `undo()` and `redo()`.
  - The history holds 32 steps. Each step is a snapshot of every frame plus
    `frame_index` and the palette. When the history is full, the oldest
    step is dropped. A new step after an undo clears redo.
- A `dirty` flag. It is set by any change and cleared by `mark_saved()`.

### 2.4 Tools (`sprite/tools.lua`)

These are pure functions on a doc.

- **Pencil:** sets the cell under the pointer to the selected key while the
  pointer is held. It joins successive samples with a line, so a fast drag
  leaves no gaps.
- **Eraser:** the same as Pencil, using `.`.
- **Fill:** a 4-way flood fill from the tapped cell. It replaces the
  connected region of that cell's key, including `.`. Filling with the same
  key is a no-op.
- **Picker:** selects the key of the tapped cell. A `.` cell selects the
  Eraser instead.
- **Line:**
  - the press sets the start point;
  - while the pointer is held, a preview line is drawn over the canvas
    without changing the doc;
  - the release commits a Bresenham line, inclusive of both ends.
- **Mirror:** when on, every cell that Pencil, Eraser or Line sets also
  sets `(sprite_w − 1 − x, y)`. Fill and Picker ignore it.

Each press-to-release gesture on the canvas is one undo step.

### 2.5 Palette and colour picker (`sprite/picker.lua`)

- **Default palette:**
  - `0` black;
  - `1`–`9` and `a`–`c`: twelve `AcidPalette.hue` steps;
  - `d` dark grey, `e` light grey, `f` white.
- **Choosing and editing a colour:**
  - Tapping a swatch selects its key.
  - Tapping the swatch that is already selected opens the picker over the
    right column.
  - The picker is a grid of 12 hues × 4 shades (full, ¾, ½ and ¼
    brightness) plus 8 greys.
  - Tapping a colour calls `set_color` as one undo step and closes the
    picker. ESC or a tap outside the picker closes it without a change.
- **Saving:** every palette slot is written, including unused ones, so
  edited colours survive a save.

### 2.6 Commands and keys

ESC, or the `≡` button at the left of the frame bar, toggles a command
strip over the frame bar. Each command is one key or one tap, in the style
of Editor's command bar:

| Key | Command |
|---|---|
| `s` | Save to the current path. Untitled sprites go to save-as. |
| `a` | Save-as. The message line becomes a name prompt. On Enter, `.spr` is appended if missing, and the file is saved to `v3/fsroot/Home/<name>`. A name that is empty or contains `/` is rejected with a message. |
| `n` | New. It asks for size `8`, `1`(6) or `3`(2), and creates a blank one-frame sprite with the default palette. |
| `u` / `r` | Undo / redo. |
| `q` | Close. |

`n` and `q` on a sprite with unsaved changes first show
`unsaved: new again to discard` (or `close`), and act on the second press.
Taps are ignored while a prompt is open; ESC cancels it.

Tapping the fps item on the frame bar steps through 2, 4, 6, 8, 12, 15, 20
and 30, wrapping round.

Outside the strip, these keys select tools and act directly:

- `p` Pencil, `f` Fill, `e` Eraser, `i` Picker, `l` Line, `m` toggles
  Mirror;
- `,` and `.` go to the previous and next frame;
- `space` starts and stops playback.

### 2.7 Playback

While playing:

- the preview and the canvas step through the frames at `fps`;
- editing is disabled;
- the frame bar shows `stop`.

Any canvas tap or `space` stops playback on the frame that is showing.

### 2.8 Touch handling

Every tap-to-act control acts once per press and ignores the rest of the
hold: buttons, swatches, frame-bar items, commands and picker cells.
Pencil and Eraser intentionally act on every held sample.

### 2.9 Drawing cost

A full redraw happens on create, resize, load, new, a frame change and
picker open/close. Any other edit redraws only the cells it changed and
the preview. At most 1024 cells are drawn per frame, which is within what
the existing Lua apps draw.

## 3. Sample sprite

`v3/fsroot/Home/acid_ship.spr` is a 16×16 ship with 2 frames, in a
flickering-exhaust style. It is there so the app has something to open.

## 4. Testing

### 4.1 Lua suites

The suites go in `v3/tools`, each registered in
`crates/acid-lua/tests/game_tests.rs`.

- **`test_acid_sprite_format.lua`:**
  - `parse` accepts the sample and minimal files;
  - every rule in §1.1 is rejected with the correct line number;
  - limits at the edges: 1 and 32, 8 and 9 frames, 16 and 17 palette
    entries;
  - comments, blank lines and CRLF are handled;
  - the `serialize` fixed point holds;
  - `load` on a missing path returns `nil` plus a message.
- **`test_sprite_doc.lua`:**
  - Pencil, Eraser and Mirror;
  - Line in all 8 directions, including both endpoints;
  - Fill stops at boundaries and fills `.` regions;
  - Picker;
  - the frame add, dup and del limits;
  - undo and redo across strokes, frame operations and palette edits;
  - the 32-step cap;
  - a new step clears redo;
  - `dirty` and `mark_saved`.
- **`test_sprite_app.lua`:**
  - **Opening:** a launch argument path loads that file. A bad file falls
    back to new and shows the error.
  - **Hit-testing:** canvas taps map to the right cells at the default size
    and after `resize_app` to the minimum and to something larger.
  - **Drawing bounds:** `drawn_inside_window()` and `drawn_text_clear()`
    hold at both sizes.
  - **Picker:** a second tap on a swatch opens it, and choosing a colour
    sets that slot.
  - **Saving:** save-as writes exactly `AcidSprite.serialize(doc)` to
    `v3/fsroot/Home/<name>.spr`. A held tap on a button acts once.
  - **Playback:** it advances frames and blocks edits.
- **File Manager suite:** a `.spr` row spawns `v3/apps/sprite.lua` with the
  file's path.

### 4.2 Rust

A test parses every `.spr` file under `v3/fsroot` by running
`AcidSprite.parse` through acid-lua, and fails if any of them is rejected.

### 4.3 Golden frame

`sprite_paint.ppm` shows Sprite Paint with `acid_ship.spr` open, on the
desktop at 640×480. The user approves it as a PNG before it is committed.

### 4.4 Manual check by the user

- Open Sprite Paint from the Menu and draw with each tool, including
  Mirror.
- Edit a palette colour.
- Add frames and play them.
- Save-as, reopen the file from File Manager, and resize the window.

## 5. Docs

- `04-graphics.md` §4.6: a "Sprite files" subsection covering the format
  and `AcidSprite.load`, `parse` and `serialize`, with the two-line game
  example.
- `09-api-reference.md`: the three new `AcidSprite` functions.
- `01-getting-started.md`: one line about Sprite Paint.

## Risks

- **Lua redraw cost at large window sizes.** It is bounded by 1024 cells
  plus the preview. Partial redraws keep a pencil drag cheap.
- **Format drift between the editor and games.** Both use the same library
  functions, and the fixed-point test pins the format.
