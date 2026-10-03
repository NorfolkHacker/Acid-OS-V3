pcall(function() table.remove(setmetatable({}, {__len = function() return math.maxinteger end}), 1) end)
pcall(function() table.insert(setmetatable({}, {__len = function() return math.maxinteger - 1 end}), 1, 0) end)
while true do end
