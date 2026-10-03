-- Touch editing: tap to place the cursor, drag to select, tap the gutter
-- for a whole line, tap the status line for command mode, tap a command
-- on the strip to run it.
--
-- The reference editor this one learns from is keyboard-only. The
-- hardware target is a tablet, so this is the half that actually matters
-- there -- and it is the same command surface the keyboard drives, not a
-- second one bolted on.
--
-- Press and release are tracked explicitly rather than with the usual
-- press-once-per-hold guard, because dragging is exactly the case that
-- guard exists to suppress. The one-shot targets -- the strip and the
-- status line -- keep the guard, via self.tap_consumed.
--
-- A mixin of EditorApp (see mixin() in editor.lua): its functions are
-- copied onto the class. Geometry is read from EditorLayout and the
-- command strip's table from EditorCmd, by qualified name.
EditorTouch = {}

function EditorTouch.editor_touch(self, x, y, pressed)
  local L = EditorLayout
  if not pressed then
    self.touch_down = false
    self.tap_consumed = false
    -- Not load-bearing today (every fresh press re-initialises this
    -- anyway, via `fresh or` in touch_gutter), but leaving a gesture
    -- flag set across gestures is exactly the shape that caused the
    -- stale-mark bug this flag exists to prevent -- clear it here too
    -- so there is never a lingering "anchored" claim from a gesture
    -- that has already ended.
    self.gutter_anchored = false
    return false
  end

  local fresh = not self.touch_down
  self.touch_down = true

  -- One-shot targets first, and only on the press that started the
  -- hold: holding a finger on the strip must not re-run its command
  -- every frame.
  if fresh then
    if self:touch_command_strip(x, y) then return true end
    if self:touch_status(x, y) then return true end
  end
  if self.tap_consumed then return false end
  if y < L.TEXT_Y or y >= L.STATUS_Y then return false end
  -- A prompt (find/goto/save-as) is modal: cmd_prompt_submit reads
  -- buf.cx/buf.cy to decide where the pending command acts from, so a
  -- tap that quietly moved the cursor underneath it would corrupt that
  -- command with no visible sign anything happened, and a drag would
  -- toggle a mark underneath the modal too. Ignoring taps in the
  -- text/gutter while a prompt is active is simplest, and matches the
  -- existing rule that the strip itself also refuses a tap while a
  -- prompt is up (touch_command_strip, above).
  if self:cmd_prompt_active() then return false end

  -- Reaching here means the tap/drag lands on the text or the gutter --
  -- real editing surface, on the same footing as any non-ESCAPE keypress
  -- on the keyboard path (editor.lua's on_key, which clears this on every
  -- key but ESCAPE). Left uncleared here, quit_armed survived a whole
  -- touch edit: tap q (arms), tap/drag in the text or gutter to actually
  -- edit the buffer, reopen the strip, tap q again -- and the window
  -- would close and discard those edits, because nothing on the touch
  -- path ever disarmed it. On the keyboardless Tab5 target this touch
  -- path is the primary (often only) way to edit, so this is the
  -- confirmation actually mattering there. The strip and the status line
  -- are deliberately NOT covered by this clear -- see their own call
  -- sites above: the strip already disarms via cmd_run for any letter
  -- but "q", and the status line is the only way to cancel a prompt with
  -- no keyboard, so it must stay untouched by this.
  self.quit_armed = false
  if x < L.GUTTER_W then
    return self:touch_gutter(y, fresh)
  end
  return self:touch_text(x, y, fresh)
end

function EditorTouch.touch_command_strip(self, x, y)
  local L = EditorLayout
  if not self:cmd_active() then return false end
  if self:cmd_prompt_active() then return false end
  local sy = self:cmd_strip_y()
  if y < sy or y >= L.STATUS_Y then return false end
  local row = (y - sy) // L.LINE_H
  local cells = EditorCmd.CMD_ROWS[row + 1]
  if cells == nil then return true end
  local col = (x - 2) // (EditorCmd.CMD_CELL_CHARS * L.CHAR_W)
  local cell = cells[col + 1]
  if cell == nil or col < 0 then
    self:cmd_close()
  else
    self:cmd_close()
    self:cmd_run(cell[1])
  end
  self.tap_consumed = true
  return true
end

function EditorTouch.touch_status(self, x, y)
  if y < EditorLayout.STATUS_Y then return false end
  -- Deliberately NOT guarded by cmd_prompt_active, unlike the strip,
  -- text and gutter. On the target hardware there is no keyboard and
  -- so no ESC: a tap on the status line is the ONLY way to cancel a
  -- find/goto/save-as prompt that was opened by tapping. Blocking this
  -- target while a prompt is active would make prompts unescapable on
  -- the device this editor is for.
  if self:cmd_active() then
    self:cmd_close()
  else
    self:cmd_open()
  end
  self.tap_consumed = true
  return true
end

function EditorTouch.touch_gutter(self, y, fresh)
  local L = EditorLayout
  local row = (y - L.TEXT_Y) // L.LINE_H + self.scroll_y
  -- Same dead band as touch_text: the gutter only ever DRAWS
  -- visible_lines rows (draw_gutter stops at that same limit), so a tap
  -- in the sliver just above STATUS_Y can compute a row that is real
  -- (< line_count, so the check below alone wouldn't catch it) but was
  -- never on screen. Clamp to the last visible row BEFORE the
  -- line_count check, same order as touch_text, so a tap there selects
  -- the last line the user could actually see instead of one further
  -- down that they couldn't.
  local max_visible_row = self.scroll_y + self:visible_lines() - 1
  if row > max_visible_row then row = max_visible_row end
  -- The out-of-range bounds check has to come before we can decide
  -- whether this row is usable, but it must NOT be allowed to skip
  -- initialisation for the rest of the hold: a hold's first event can
  -- land below the last line (row invalid, we return here) while a
  -- later event in the SAME hold lands on a real row with `fresh`
  -- already false. Without gutter_anchored, that later event would
  -- fall straight into the "extend" branch below and reuse whatever
  -- mark a previous, unrelated gesture left behind -- selections
  -- deliberately persist across releases, so a stale one is sitting
  -- right there waiting to be extended by a gesture that never
  -- anchored its own.
  if row >= self.buf:line_count() then
    if fresh then self.gutter_anchored = false end
    return false
  end
  if fresh or not self.gutter_anchored then
    self.buf:clear_mark()
    self.buf:set_cursor(0, row)
    self.buf:toggle_mark()
    self.gutter_anchored = true
  end
  -- Extending down the gutter selects whole lines: the mark stays at the
  -- start of the first, the cursor runs to the end of the current.
  self.buf:set_cursor(#self.buf:line(row), row)
  self:ensure_scroll()
  return true
end

function EditorTouch.touch_text(self, x, y, fresh)
  local L = EditorLayout
  local row = (y - L.TEXT_Y) // L.LINE_H + self.scroll_y
  -- A window this tall only ever DRAWS visible_lines rows before
  -- STATUS_Y; a few px of rounding in the sliver just above the status
  -- line computes a real but undrawn line index (visible_lines rows
  -- means rows scroll_y..scroll_y+visible_lines-1 are ever on screen).
  -- Clamping to line_count alone isn't enough on a buffer longer than
  -- the screen -- the computed row is still a valid line, just one
  -- nobody can see, and landing the cursor there makes ensure_scroll
  -- yank the whole viewport from a tap that looked like it hit nothing.
  local max_visible_row = self.scroll_y + self:visible_lines() - 1
  if row > max_visible_row then row = max_visible_row end
  if row >= self.buf:line_count() then row = self.buf:line_count() - 1 end
  local col = (x - L.TEXT_X) // L.CHAR_W + self.scroll_x
  if col < 0 then col = 0 end
  -- A continuation with no anchor is not really a continuation: drag_from
  -- is only ever assigned in the `fresh` branch below, and two reachable
  -- routes land here on a "continuation" (fresh == false) where it was
  -- never set. (1) A press starts in the gutter BELOW the last line --
  -- touch_gutter returns early without touching drag_from -- and then
  -- drags into the text; on any file shorter than the gutter's ~25
  -- visible rows (including the default notes.txt) most of the gutter is
  -- below the last line, so this is routine, not a corner case. (2) A
  -- press-and-hold starts in the text while a find/goto/save-as prompt is
  -- open (editor_touch's prompt guard, above, returns before touch_text
  -- ever runs, so drag_from is never assigned), the prompt is cancelled
  -- with ESC while still held, and the same hold then moves. Indexing a
  -- nil drag_from here would raise, uncaught by on_touch, which takes the
  -- whole app down and the window with it -- unsaved work and all.
  -- Treating a missing anchor as a fresh press is the fix: it anchors
  -- right here, at the point the drag is first actually seen, instead of
  -- failing on an anchor that was never set.
  if self.drag_from == nil then fresh = true end
  if fresh then
    self.buf:clear_mark()
    self.buf:set_cursor(col, row)
    self.drag_from = { self.buf.cx, self.buf.cy }
    self:ensure_scroll()
    return true
  end
  -- Continuing a hold: this is a drag, so anchor a mark at wherever the
  -- press landed and let the cursor run.
  if not self.buf:mark_set() then
    self.buf:set_cursor(self.drag_from[1], self.drag_from[2])
    self.buf:toggle_mark()
  end
  self.buf:set_cursor(col, row)
  self:ensure_scroll()
  return true
end
