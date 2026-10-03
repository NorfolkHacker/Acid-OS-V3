local xp = xpcall
local h = function() return 1 end
pcall = function() return false end
while true do xp(function() while true do end end, h) end
