-- Headless tests for About (apps/about.lua): it reads Help/about.txt
-- (falling back to one line), splits it on newlines (trailing
-- blank lines dropped), and draws lines cut to 26 chars until the next
-- would pass the bottom edge.

local G = GAME

group("text")
eq(G.lines, { "Acid OS v3" }, "a missing about.txt falls back to one line")
FS["v3/fsroot/Help/about.txt"] = "line one\nline two\n\nlast\n\n"
G:on_create()
eq(G.lines, { "line one", "line two", "", "last" }, "lines split on newlines")
local long = {}
for i = 1, 20 do long[i] = string.rep("x", 30) end
FS["v3/fsroot/Help/about.txt"] = table.concat(long, "\n")
G:on_create()
TEXTS = {}
G:redraw()
eq(#TEXTS, 13, "drawing stops at the window's bottom edge")
eq(TEXTS[1], string.rep("x", 26), "each line is cut to 26 characters")
