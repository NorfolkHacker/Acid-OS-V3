-- Geometry shared between EditorApp and its mixins (EditorCmd, EditorTouch).
--
-- The constants live on this one table, and every module reads them from
-- here (EditorLayout.LINE_H) rather than reaching into its includer.
EditorLayout = {}

-- Must match the kernel's title bar height.
-- 420x280 at Normal gives 25 lines of 65 columns; the window is what
-- acid_window_size reports.
local CW, CH = acid_font_size()
EditorLayout.TITLE_BAR_H = 16
EditorLayout.LINE_H = CH + 2
EditorLayout.CHAR_W = CW

EditorLayout.TEXT_Y = EditorLayout.TITLE_BAR_H

EditorLayout.GUTTER_CHARS = 4
EditorLayout.GUTTER_W = EditorLayout.GUTTER_CHARS * EditorLayout.CHAR_W
EditorLayout.TEXT_X = EditorLayout.GUTTER_W + 2

-- Everything derived from the window size. Called once below, then again
-- by EditorApp:on_resize.
--
-- The status line sits at the bottom of the window: command mode raises
-- its strip above it, and a command surface that grows upward from the
-- bottom edge doesn't push the text you're looking at around.
function EditorLayout.compute(w, h)
  EditorLayout.WINDOW_W = w
  EditorLayout.WINDOW_H = h
  EditorLayout.STATUS_Y = h - EditorLayout.LINE_H
  -- The scroll bar's column, just inside the right border (lib/
  -- acid_scrollbar.lua, which the manifest loads first); text stops
  -- short of it.
  EditorLayout.BAR_X = w - 1 - AcidScrollbar.WIDTH
end
EditorLayout.compute(acid_window_size())

-- The editor's own source, plus the libs its manifest loads (acid_app.lua
-- and acid_scrollbar.lua: every app loads the first, and a bad save in
-- either bricks the editor, so it must not be able to break itself). Both
-- save_file (EditorApp) and cmd_run_file (EditorCmd, a mixin) need this, so
-- it lives here, in the one module both sides share.
--
-- Anchored to the two roots this file can actually be reached through
-- -- v3/apps (canonical) and v3/fsroot/App (the live symlink to it) --
-- not a bare filename-tail suffix. A bare suffix match ("ends with
-- /editor.lua") would be a real bug: a user's OWN script at
-- v3/fsroot/Home/editor.lua, or v3/fsroot/Home/lib/acid_app.lua, also ends
-- with those letters, so it would get a spurious .bak on every save and
-- ESC ! would refuse to run it with a
-- message about the EDITOR's own source -- baffling on a text-editing OS
-- where naming a script "editor.lua" is entirely plausible. Stripping a
-- known root first and comparing the REST against the exact relative
-- path list means a file under fsroot/Home can never match no matter
-- what it's named, while both real forms of each own-source file still do.
-- Called as a method (self:own_source(path)) once mixed into EditorApp.
EditorLayout.OWN_SOURCE_ROOTS = { "v3/apps/", "v3/fsroot/App/" }
EditorLayout.OWN_SOURCE_RELATIVE_PATHS = {
  "editor.lua",
  "editor/buffer.lua",
  "editor/hl.lua",
  "editor/cmdbar.lua",
  "editor/layout.lua",
  "editor/touch.lua",
  "lib/acid_app.lua",
  "lib/acid_scrollbar.lua",
}

function EditorLayout.own_source(self, path)
  for _, root in ipairs(EditorLayout.OWN_SOURCE_ROOTS) do
    if path:sub(1, #root) == root then
      local rel = path:sub(#root + 1)
      for _, known in ipairs(EditorLayout.OWN_SOURCE_RELATIVE_PATHS) do
        if known == rel then return true end
      end
    end
  end
  return false
end
