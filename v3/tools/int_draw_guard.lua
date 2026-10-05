-- Loaded after game_test_env.lua, before the app: wraps the drawing calls
-- so a suite can check none got a fractional number. The real bindings
-- take whole pixels and raise on 3.5, which the plain stubs would let
-- through. NON_INT collects "name(arg n)" for each offender.

NON_INT = {}

for _, name in ipairs({ "acid_fill_rect", "acid_fill_circle", "acid_draw_line", "acid_draw_text", "acid_fill_triangle" }) do
  local real = _G[name]
  _G[name] = function(...)
    local args = table.pack(...)
    for i = 1, args.n do
      local v = args[i]
      if type(v) == "number" and math.type(v) ~= "integer" then
        NON_INT[#NON_INT + 1] = name .. "(arg " .. i .. " = " .. v .. ")"
      end
    end
    return real(...)
  end
end
