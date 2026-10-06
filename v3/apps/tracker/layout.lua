-- TrkLayout: where Acid Tracker draws, for a window size. Top to bottom:
-- the status line, channel headings, the pattern grid, the orders panel
-- (one line per channel), the instrument panel and the message / command
-- line. Text is always 6x8 (the app isn't font-scalable).

TrkLayout = {}
TrkLayout.TITLE_H = 16
TrkLayout.CH_W, TrkLayout.CH_H = 6, 8
TrkLayout.MARGIN = 4
TrkLayout.CELL_CHARS = 15        -- "C-4 01 4 22 E-4"
TrkLayout.COL_CHARS = 16         -- a cell and a gap
TrkLayout.ROWNUM_CHARS = 3       -- "0A "
TrkLayout.ORDER_LINES = 4
TrkLayout.INS_LINES = 3
-- Where each cursor column sits inside a cell, and how wide it is.
TrkLayout.SLOT_CHARS = { note = 0, inst = 4, cmd = 7, param = 9, note2 = 12 }
TrkLayout.SLOT_W = { note = 3, inst = 2, cmd = 1, param = 2, note2 = 3 }

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
  L.rows = math.max(1, (L.ord_y - 2 - L.grid_y) // S.CH_H)
  L.ch_x = {}
  for ch = 1, 4 do L.ch_x[ch] = L.x + (S.ROWNUM_CHARS + (ch - 1) * S.COL_CHARS) * S.CH_W end
  L.text_cols = (w - 2 * L.x) // S.CH_W
  return L
end
