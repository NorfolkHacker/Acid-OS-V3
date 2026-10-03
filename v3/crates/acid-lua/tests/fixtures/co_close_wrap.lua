coroutine.wrap(function()
  local c <close> = setmetatable({}, {__close = function() while true do end end})
  while true do end
end)()
