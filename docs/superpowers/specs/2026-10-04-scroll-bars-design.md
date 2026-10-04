# Scroll Bars for Editor, Terminal and Load Cart — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

File Manager already has a right-hand scroll bar, built from the shared
`AcidScrollbar` library (`apps/lib/acid_scrollbar.lua`). Three more windows
get the same bar:

- **Editor**, for the file;
- **Terminal**, which also gains scroll-back;
- **Load Cart**, for its cart list.

## Decisions

| Question | Choice |
|---|---|
| Which windows | Editor, Terminal and Load Cart. |
| How | Each app uses `AcidScrollbar` the way File Manager does. There are no library, kernel or API changes. |
| Editor | The bar moves the view only. The cursor stays where it is, even off-screen. The next key that moves or edits the cursor scrolls back to it. |
| Terminal | Adds scroll-back. Typing and new output jump back to the bottom. |
| Load Cart | The same as File Manager's listing. |

## Out of scope

- Mouse-wheel scrolling, which needs a new platform event.
- PageUp and PageDown keys.
- Horizontal scroll bars. Editor keeps its existing horizontal scrolling,
  which follows the cursor.
- Changes to `AcidScrollbar` itself.
- A cap on Terminal's scroll-back length.

## 1. Shared behaviour

All three apps follow File Manager's pattern:

- **Placement:** the bar is `AcidScrollbar.WIDTH` (6) px wide. It sits at
  `x = window_w − 1 − WIDTH`, just inside the right border. Its track
  spans exactly the rows it scrolls.
- **Visibility:** it is drawn only when `AcidScrollbar.needed(total, visible)`
  is true. Text and row backgrounds always stop short of the bar's column,
  whether or not the bar is showing, so the layout doesn't jump.
- **Paging:** a press on the track above or below the thumb moves one
  screenful, using `AcidScrollbar.press`.
- **Dragging:** a press on the thumb starts a drag, and held touches follow
  it through `AcidScrollbar.drag`. A release ends the drag.
- **Touch handling:**
  - a press on the bar is never passed on to the app's own touch handling;
  - a held touch that started on the bar drags it and nothing else;
  - the drag acts on every held sample, while every other control keeps its
    press-once guard.
- **Resize and font:**
  - the bar's geometry is recomputed in each app's existing layout step, so
    it follows window resizes and the Large font;
  - the 6 px width is not scaled, the same as File Manager.
- **Offset clamping:** the scroll offset is clamped to
  `0 .. AcidScrollbar.max_offset(total, visible)` whenever the number of
  rows or the visible row count changes.

## 2. Editor

- **Rows scrolled:** buffer lines.
  - `total` is `buf:line_count()`, or the buffer's equivalent call;
  - `visible` is `visible_lines()`;
  - the offset is `scroll_y`.
- **Track:** from `EditorLayout.TEXT_Y` to the top of the status line
  (`STATUS_Y`). When the command strip is open it draws over the bottom of
  the text area, and over the track with it, as it already does for text.
- **Width:** `visible_cols()` loses `WIDTH + 1` px, so text, selection
  highlights and the cursor stop before the bar. Click-to-cursor maps x
  against the narrower area.
- **Paging and dragging** change only `scroll_y`. The cursor (`buf.cx`,
  `buf.cy`) and the selection are unchanged.
- **Snapping back to the cursor:** any key that edits or moves the cursor
  already calls `ensure_scroll()`, which brings the cursor back into view.
  A redraw that doesn't come from such a key keeps the scrolled view.
- **Bar presses:** a press on the bar doesn't move the cursor, start a
  selection, or close the command strip.

## 3. Terminal

- **Rows scrolled:** `self.lines`.
  - `total` is `#self.lines`;
  - `visible` is `visible_lines()`;
  - the offset is a new field, `scroll`, holding the index of the first
    line shown.
  - "At the bottom" means `scroll == max_offset`.
- **Track:** from `TITLE_BAR_H` to the top of the input line. The input
  line stays full width.
- **Width:** scroll-back text is cut to the columns that fit beside the bar.
  The input line and its cursor keep the full `COLS`.
- **Following the output:** while at the bottom, each new line keeps the
  view at the bottom. This is what happens today.
- **Jumping back to the bottom:** any key press does it, as does any new
  output, such as `submit` or an egg printing.
- **Touch:** Terminal gains `on_touch`, which handles only the bar. Touches
  elsewhere still do nothing.
- **Large font and resize:** after a resize, the offset is re-clamped, and
  a view that was at the bottom stays at the bottom.

## 4. Load Cart

- **Rows scrolled:** the list rows.
  - `total` is the row count;
  - `visible` is `visible_rows()`;
  - the offset is `scroll`.
- **Track:** from the top of the list to the bottom of the list area,
  between the header and the footer.
- **Width:** list rows and their labels stop short of the bar.
- **Selection:** paging and dragging move `scroll` without changing
  `selected`. A key press that moves the selection calls `ensure_scroll()`
  as it does now. The header's `(n/N)` counter is kept.
- **Touch:** a press on the bar doesn't select or open a row.

## 5. Testing

Each app gets a headless Lua suite (or new groups in its existing suite)
covering:

- **Geometry:** the bar's x, y and h at the opening size and after
  `resize_app`. Editor and Terminal are also checked at Large.
- **Visibility:** no bar is drawn when every row fits, and one is drawn
  when they don't.
- **Fit:** `drawn_inside_window()` and `drawn_text_clear()` hold. No text
  rectangle reaches the bar's column.
- **Paging:** a press below the thumb moves one screenful.
- **Dragging:** a held drag to the bottom of the track reaches
  `max_offset`. A press on the bar has no other effect.
- **Editor:** dragging leaves the cursor and selection unchanged. The next
  arrow key snaps back to the cursor.
- **Terminal:**
  - scrolling up shows older lines;
  - a key press or a `submit` returns to the bottom;
  - while at the bottom, new output follows.
- **Load Cart:** paging leaves `selected` unchanged. A key that moves the
  selection makes it visible again.

Every new or changed suite must fail against the current code. Assertion
counts are pinned in `game_tests.rs` as usual.

### Golden frames

No existing golden frame shows Editor, Terminal or Load Cart, so none
should change. If one does, it is shown to the user as a PNG before it is
committed.

## 6. Docs

- `docs/manual-v3/01-getting-started.md`: one line saying Editor, Terminal
  and Load Cart scroll with a bar, and Terminal keeps its scroll-back.
- `docs/manual-v3/09-api-reference.md` (the `AcidScrollbar` entry): check
  that nothing contradicts the new uses. Add text only if something is
  missing.

## Risks

- **Editor's width maths.** Text, selection, cursor and click-to-cursor all
  use the column count, and one missed spot shows up as text under the
  bar. The "nothing reaches the bar's column" check and the
  click-to-cursor tests cover this.
- **Touch routing order.** A bar press must win over the app's own
  handling. Each app's suite checks that a bar press has no other effect.
