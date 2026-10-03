local co = coroutine.create(function()
  local c <close> = setmetatable({}, {__close = function() while true do end end})
  while true do end
end)
coroutine.resume(co)
