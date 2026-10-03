local t = {}
table.insert(t, "a")
table.insert(t, "c")
table.insert(t, 2, "b")
assert(#t == 3 and t[1] == "a" and t[2] == "b" and t[3] == "c")
assert(table.remove(t) == "c")
assert(table.remove(t, 1) == "a")
assert(#t == 1 and t[1] == "b")
-- A __len returning a numeric string is used the way luaL_len uses it.
local s = setmetatable({10, 20}, {__len = function() return "2" end})
table.insert(s, 30)
assert(rawget(s, 3) == 30)
local closed = 0
local closer = setmetatable({}, {__call = function() closed = closed + 1 end})
do
  local x <close> = setmetatable({}, {__close = closer})
end
assert(closed == 1)
acid_fill_rect(0, 0, 10, 10, 0xFF0000)
while acid_poll_event(50) ~= "close" do end
