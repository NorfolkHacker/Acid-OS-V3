coroutine.wrap(function()
  local c <close> = setmetatable({}, {__close = setmetatable({}, {__call = function() while true do end end})})
  while true do end
end)()
