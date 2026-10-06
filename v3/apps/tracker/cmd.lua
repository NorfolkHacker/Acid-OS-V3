-- TrkCmd: Acid Tracker's Esc command line. Parsing only; TrackerApp runs
-- the commands. Numbers are whole and must fit, song names stay under
-- Home.

TrkCmd = {}
TrkCmd.USAGE = {
  w = "w [name]", o = "o name", new = "new", speed = "speed 1-31", len = "len 1-64",
  title = "title text", ins = "ins N  or  ins N script PATH NAME", name = "name text",
  arp = "arp [a [b [c]]]", donor = "donor 1-4", q = "q",
}
-- Fewest and most arguments each command takes.
local ARGS = {
  w = { 0, 1 }, o = { 1, 1 }, new = { 0, 0 }, speed = { 1, 1 }, len = { 1, 1 }, title = { 0, 99 },
  ins = { 1, 4 }, name = { 0, 99 }, arp = { 0, 3 }, donor = { 1, 1 }, q = { 0, 0 },
}

-- A typed line -> { name, args, rest } (rest is everything after the
-- name, as typed), or nil and a message ("" for an empty line).
function TrkCmd.parse(line)
  local words = {}
  for w in line:gmatch("%S+") do words[#words + 1] = w end
  local name = table.remove(words, 1)
  if not name then return nil, "" end
  local n = ARGS[name]
  if not n then return nil, "unknown command: " .. name end
  if #words < n[1] or #words > n[2] then return nil, "usage: " .. TrkCmd.USAGE[name] end
  return { name = name, args = words, rest = line:match("^%s*%S+%s*(.-)%s*$") }
end

-- A whole number, or nil (not a number, or too big to hold).
function TrkCmd.int(s)
  if s and s:match("^[+-]?%d+$") then return math.tointeger(tonumber(s)) end
end

-- One or two hex digits, or nil.
function TrkCmd.hex(s)
  if s and s:match("^%x%x?$") then return tonumber(s, 16) end
end

-- A song name -> its path under Home with .trk added, or nil if it would
-- leave Home or isn't a plain name.
function TrkCmd.home_path(name)
  if name == "" or name:sub(1, 1) == "/" or name:sub(-1) == "/" or name:find("//", 1, true) then return nil end
  for seg in name:gmatch("[^/]+") do
    if seg == "." or seg == ".." then return nil end
  end
  if name:sub(-4) ~= ".trk" then name = name .. ".trk" end
  return "v3/fsroot/Home/" .. name
end
