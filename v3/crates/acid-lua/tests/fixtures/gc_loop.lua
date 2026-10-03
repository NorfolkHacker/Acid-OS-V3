local ok = pcall(setmetatable, {}, {__gc = function() while true do end end})
acid_fill_rect(0, 0, 4, 4, 0xFF0000)
while true do
  setmetatable({}, {__gc = function() while true do end end})
  collectgarbage()
end
