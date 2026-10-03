pcall(function() table.insert(setmetatable({}, {__len = function() return "1000000000000" end}), 1, 0) end)
pcall(function() table.remove(setmetatable({}, {__len = function() return "1000000000000" end}), 1) end)
while true do end
