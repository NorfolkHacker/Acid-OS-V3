while true do
  pcall(function()
    local c <close> = setmetatable({}, {__close = function() error("nope") end})
    while true do end
  end)
  pcall(function()
    local c <close> = setmetatable({}, {__close = function() error({}) end})
    while true do end
  end)
end
