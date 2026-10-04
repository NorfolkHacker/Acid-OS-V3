-- Every .spr shipped in v3/fsroot/Home parses. SPR is filled in by
-- game_tests.rs: { { name, text }, ... }.
for _, f in ipairs(SPR) do
  local s, err = AcidSprite.parse(f[2])
  ok(s ~= nil, f[1] .. " parses" .. (err and (": " .. err) or ""))
end
