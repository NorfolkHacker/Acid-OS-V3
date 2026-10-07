-- TrkLayout: where Acid Tracker draws, for a window size. Top to bottom:
-- the status line, track headings, the pattern grid, the tracks' scroll
-- bar, the orders panel, the instrument panel and the message / command
-- line. Eight tracks don't fit a small window, so the grid shows as many
-- as fit (`visible`) and scrolls sideways. Text is always 6x8 (the app
-- isn't font-scalable).

TrkLayout = {}
TrkLayout.TITLE_H = 16
TrkLayout.CH_W, TrkLayout.CH_H = 6, 8
TrkLayout.ROW_H = 10             -- grid rows only: 8 px text with a gap, so rows don't touch
TrkLayout.MARGIN = 4
TrkLayout.CELL_CHARS = 11        -- "C-4 01 4 22"
TrkLayout.COL_CHARS = 12         -- a cell and a gap
TrkLayout.ROWNUM_CHARS = 3       -- "0A "
TrkLayout.ORDER_LINES = 2        -- the order list, then its keys
TrkLayout.INS_LINES = 3
-- Where each cursor column sits inside a cell, and how wide it is.
TrkLayout.SLOT_CHARS = { note = 0, inst = 4, cmd = 7, param = 9 }
TrkLayout.SLOT_W = { note = 3, inst = 2, cmd = 1, param = 2 }

function TrkLayout.compute(w, h)
  local S = TrkLayout
  local L = { w = w, h = h }
  L.x = 1 + S.MARGIN
  L.status_y = S.TITLE_H + 3
  L.head_y = L.status_y + S.CH_H + 2
  L.grid_y = L.head_y + S.CH_H + 1
  L.msg_y = h - 2 - S.CH_H
  L.ins_y = L.msg_y - 2 - S.INS_LINES * S.CH_H
  L.ord_y = L.ins_y - 2 - S.ORDER_LINES * S.CH_H
  L.bar_y = L.ord_y - 3 - AcidScrollbar.WIDTH
  L.rows = math.max(1, (L.bar_y - 1 - L.grid_y) // S.ROW_H)
  L.text_cols = (w - 2 * L.x) // S.CH_W
  L.tracks_x = L.x + S.ROWNUM_CHARS * S.CH_W
  -- The last cell needs no gap after it.
  L.visible = math.max(1, math.min(TrkSong.TRACKS, (L.text_cols - S.ROWNUM_CHARS + 1) // S.COL_CHARS))
  L.bar_w = w - L.x - L.tracks_x
  return L
end

-- The x of the i-th track on screen (0-based from the first one shown).
function TrkLayout.track_x(L, i)
  return L.tracks_x + i * TrkLayout.COL_CHARS * TrkLayout.CH_W
end
