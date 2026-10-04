-- Shared test environment for the game apps. Loaded BEFORE the app under
-- test: each app file ends in `<Class>:new():start()`, so the
-- AcidApp/AcidGame stubs below must exist (and must NOT enter an event
-- loop) before that line runs. Every acid_* call the games make is stubbed;
-- the audio ones record what they were called with, since "was the note
-- ever turned off" is what the game tests check. Run by
-- v3/crates/acid-lua/tests/game_tests.rs.

NOTES = {}
RECTS = {}
TEXTS = {}
TEXT_AT = {}
DRAW_CALLS = 0
CLOCK = 0
FAILS = {}
PASSES = 0

local function push(t, v) t[#t + 1] = v end

-- The screen size apps see. The suites' expectations were written for
-- v2's 640x360; a suite can load a file setting SCREEN_W/SCREEN_H before
-- this one to run at another size (screen_800x600.lua).
SCREEN_W = SCREEN_W or 640
SCREEN_H = SCREEN_H or 360
function acid_screen_size() return SCREEN_W, SCREEN_H end

function acid_play_note(voice, ona, volume) push(NOTES, { "play", voice, ona, volume }) end
function acid_trigger_arp(voice, n0, n1, n2, n3, count, rate_ms)
  push(NOTES, { "arp", voice, n0, n1, n2, n3, count, rate_ms })
end
function acid_stop_note(voice) push(NOTES, { "stop", voice }) end
function acid_configure_voice(...) end
function acid_configure_filter(...) end
function acid_configure_osc(...) end
function acid_set_ring_partner(...) end

function acid_clear_user_area() DRAW_CALLS = DRAW_CALLS + 1 end
function acid_draw_window_frame(title) DRAW_CALLS = DRAW_CALLS + 1 end
function acid_draw_window_border() DRAW_CALLS = DRAW_CALLS + 1 end
function acid_draw_text(str, x, y, fg, bg) DRAW_CALLS = DRAW_CALLS + 1; push(TEXTS, str); push(TEXT_AT, { str, x, y }) end
function acid_fill_circle(x, y, r, color) DRAW_CALLS = DRAW_CALLS + 1 end
function acid_fill_rect(x, y, w, h, color)
  DRAW_CALLS = DRAW_CALLS + 1
  push(RECTS, { x, y, w, h, color })
end
function acid_overlay_fill_rect(x, y, w, h, color) push(RECTS, { x, y, w, h, color }) end
-- The 3D calls (Lua API of acid-api): recorded, never rasterised.
LINES = {}
TRIS = {}
MESH_DRAWS = {}
MESH_NEWS = {}
MESH_FREES = {}
local mesh_next = 0
local BUILTIN_MESHES = { cube = true, pyramid = true, octahedron = true, sphere = true, torus = true }
function acid_draw_line(x1, y1, x2, y2, c) push(LINES, { "line", x1, y1, x2, y2, c }) end
function acid_fill_triangle(x1, y1, x2, y2, x3, y3, c) push(TRIS, { "tri", x1, y1, x2, y2, x3, y3, c }) end
function acid_mesh_builtin(name)
  if not BUILTIN_MESHES[name] then return nil end
  mesh_next = mesh_next + 1
  return mesh_next
end
function acid_mesh_new(points, faces)
  push(MESH_NEWS, { points, faces })
  mesh_next = mesh_next + 1
  return mesh_next
end
function acid_mesh_draw(id, x, y, size, rx, ry, rz, mode, color)
  push(MESH_DRAWS, { id, x, y, size, rx, ry, rz, mode, color })
end
function acid_mesh_free(id) push(MESH_FREES, id) end
function acid_fs_size(path)
  local f = FS[path]
  if type(f) ~= "string" then return nil, "not found" end
  return #f
end
function acid_spawn_app(path, w, h, arg) push(CALLS, { "spawn", path, w, h, arg }); return true end
function acid_now_ms() return CLOCK end

-- System-app fakes (Phase 5a: desktop, about, config, network, sysmon).
-- Tests set and read these directly; every kernel-changing call is
-- recorded in CALLS.
FS = {}
TIME = { 2026, 10, 2, 9, 5, 0 }
WINDOWS = {}
LAUNCHER = {}
CALLS = {}
WALLPAPER = true
VOLUME = 50
MEM_KB = 1234
NETWORK = { "acid-box", "192.168.1.20", true }
TASKS = {}
REFRESHES = 0
FRAMES = { composited = 0, skipped = 0 }
VOICES = 0

-- What the app under test gets from acid_font_size / acid_window_size /
-- the font setting. A suite sets WIN_W/WIN_H (and FONT_W/FONT_H for Large)
-- in its prelude, before this file loads; the defaults are Normal.
FONT_W = FONT_W or 6
FONT_H = FONT_H or 8
FONT_SCALE = FONT_SCALE or 1
function acid_font_size() return FONT_W, FONT_H end
function acid_window_size()
  assert(WIN_W and WIN_H, "this suite must set WIN_W and WIN_H in its prelude")
  return WIN_W, WIN_H
end
function acid_get_font_scale() return FONT_SCALE end
function acid_set_font_scale(n)
  push(CALLS, { "set_font_scale", n })
  if n == 1 or n == 2 then FONT_SCALE = n end
end

-- Whether everything drawn since TEXT_AT/RECTS were last emptied lies
-- inside the window, with no character cut off at an edge. Returns false
-- and what didn't fit.
function drawn_inside_window()
  for _, t in ipairs(TEXT_AT) do
    local s, x, y = t[1], t[2], t[3]
    if x < 0 or y < 0 or x + #s * FONT_W > WIN_W or y + FONT_H > WIN_H then
      return false, "text '" .. s .. "' at " .. x .. "," .. y
    end
  end
  for _, r in ipairs(RECTS) do
    if r[1] < 0 or r[2] < 0 or r[1] + r[3] > WIN_W or r[2] + r[4] > WIN_H then
      return false, "rect at " .. r[1] .. "," .. r[2] .. " size " .. r[3] .. "x" .. r[4]
    end
  end
  for _, m in ipairs(MESH_DRAWS) do
    if m[2] < 0 or m[3] < 0 or m[2] > WIN_W or m[3] > WIN_H then
      return false, "mesh centre at " .. m[2] .. "," .. m[3]
    end
  end
  return true
end

-- Whether any two strings drawn since TEXT_AT was last emptied overlap
-- (text laid out at the wrong pitch collides with its neighbours). Returns
-- false and the pair.
function drawn_text_clear()
  for i, a in ipairs(TEXT_AT) do
    for k = i + 1, #TEXT_AT do
      local b = TEXT_AT[k]
      if a[2] < b[2] + #b[1] * FONT_W and b[2] < a[2] + #a[1] * FONT_W
        and a[3] < b[3] + FONT_H and b[3] < a[3] + FONT_H then
        return false, "'" .. a[1] .. "' overlaps '" .. b[1] .. "'"
      end
    end
  end
  return true
end

function acid_fs_list(dir)
  local d = FS[dir]
  if type(d) ~= "table" then return nil, "not found" end
  local copy = {}
  for i, v in ipairs(d) do copy[i] = v end
  return copy
end
function acid_fs_read(path)
  local f = FS[path]
  if type(f) ~= "string" then return nil, "not found" end
  return f
end
LAUNCH_ARG = ""
FAIL_WRITES = {}   -- path -> true: acid_fs_write fails for it
FAIL_RENAMES = {}  -- from-path -> true: acid_fs_rename fails for it
function acid_launch_arg() return LAUNCH_ARG end
function acid_fs_write(path, data)
  push(CALLS, { "write", path })
  if FAIL_WRITES[path] then return nil, "disk full" end
  FS[path] = data
  return true
end
function acid_fs_rename(from, to)
  push(CALLS, { "rename", from, to })
  if FAIL_RENAMES[from] or type(FS[from]) ~= "string" then return nil, "not found" end
  FS[to] = FS[from]
  FS[from] = nil
  return true
end
function acid_fs_delete(path)
  push(CALLS, { "delete", path })
  if FS[path] == nil then return nil, "not found" end
  FS[path] = nil
  return true
end
function acid_local_time() return table.unpack(TIME, 1, 6) end
function acid_window_max() return 8 end
function acid_window_info(i)
  local w = WINDOWS[i]
  if not w then return end
  return table.unpack(w, 1, 6)
end
function acid_activate_window(i) push(CALLS, { "activate", i }) end
function acid_close_window(i) push(CALLS, { "close", i }); return true end
function acid_send_self_to_back() push(CALLS, { "to_back" }) end
function acid_repaint_region(x, y, w, h) push(CALLS, { "repaint", x, y, w, h }) end
function acid_launcher_register(path, name, w, h, multi, libs)
  -- The real binding takes i32s, and mlua raises on an out-of-range
  -- integer; mirror that so a huge manifest w/h is exercised here too.
  for _, v in ipairs({ w, h }) do
    if v < -2^31 or v > 2^31 - 1 then error("out of range") end
  end
  push(LAUNCHER, { path, name, w, h, multi, libs })
  return true
end
function acid_launcher_count() return #LAUNCHER end
function acid_launcher_name(i) local e = LAUNCHER[i + 1]; return e and e[2] end
function acid_launcher_path(i) local e = LAUNCHER[i + 1]; return e and e[1] end

-- Host cart folders (Phase 5d: Load Cart). CART_FS maps a host path to a
-- string (a file) or a sequence of names (a directory); CART_LINKS marks
-- symlinks, which the real bindings leave out of listings and refuse.
CART_ROOTS = {}   -- sequence of root strings acid_cart_roots returns
CART_FS = {}      -- host path -> string (file) or sequence of names (dir)
CART_LINKS = {}   -- host path -> true: a symlink, left out of listings and refused
function acid_cart_roots() local c = {} for i, r in ipairs(CART_ROOTS) do c[i] = r end return c end
function acid_cart_list(dir)
  if CART_LINKS[dir] then return nil, "bad path" end
  local d = CART_FS[dir]
  if type(d) ~= "table" then return nil, "not found" end
  local out = {}
  for _, n in ipairs(d) do if not CART_LINKS[dir .. "/" .. n] then out[#out + 1] = n end end
  table.sort(out)
  return out
end
function acid_cart_stat(path)
  if CART_LINKS[path] then return nil, "bad path" end
  local v = CART_FS[path]
  if type(v) == "table" then return "dir", 0 end
  if type(v) == "string" then return "file", #v end
  return nil, "not found"
end
function acid_cart_read(path)
  if CART_LINKS[path] then return nil, "bad path" end
  local v = CART_FS[path]
  if type(v) ~= "string" then return nil, "not found" end
  if #v > 262144 then return nil, "too big" end
  return v
end
function acid_launcher_spawn(i) push(CALLS, { "launch", i }); return true end
RESTART_OK = true  -- what acid_restart answers
function acid_restart() push(CALLS, { "restart" }); return RESTART_OK end
function acid_get_wallpaper_enabled() return WALLPAPER end
function acid_set_wallpaper_enabled(on) WALLPAPER = on; push(CALLS, { "set_wallpaper", on }) end
function acid_get_volume() return VOLUME end
function acid_set_volume(v) VOLUME = v; push(CALLS, { "set_volume", v }) end
function acid_mem_used_kb() return MEM_KB end
function acid_network_info() return table.unpack(NETWORK, 1, 3) end
function acid_refresh_tasks() REFRESHES = REFRESHES + 1; return #TASKS end
function acid_task_count() return #TASKS end
function acid_task_info(i)
  local t = TASKS[i + 1]
  if not t then return end
  return table.unpack(t, 1, 3)
end
function acid_composited_frames() return FRAMES.composited end
function acid_skipped_frames() return FRAMES.skipped end
function acid_active_voice_count() return VOICES end

-- A minimal AcidApp: just what the games use.
AcidApp = {}
AcidApp.__index = AcidApp
function AcidApp:extend(class_name)
  local cls = setmetatable({}, { __index = self })
  cls.__index = cls
  cls.class_name = class_name
  return cls
end
function AcidApp:new() return setmetatable({}, self) end
function AcidApp:window_title() return "Test" end
function AcidApp:focused() return true end
function AcidApp:on_create() end
function AcidApp:on_touch(x, y, pressed) end
function AcidApp:on_key(code, pressed) end
function AcidApp:on_idle() end
function AcidApp:on_resize(w, h) end
function AcidApp:on_destroy() end
AcidApp.FSROOT_APP_PREFIX = "v3/fsroot/App/"
AcidApp.CANONICAL_APP_PREFIX = "v3/apps/"
function AcidApp:canonical_app_path(path)
  local prefix = AcidApp.FSROOT_APP_PREFIX
  if path:sub(1, #prefix) ~= prefix then return path end
  return AcidApp.CANONICAL_APP_PREFIX .. path:sub(#prefix + 1)
end
function AcidApp:quit() self.running = false end
function AcidApp:start()
  self:on_create()
  GAME = self
end

AcidGame = AcidApp:extend("AcidGame")
AcidGame.TICK_MS = 50
function AcidGame:on_tick() end
function AcidGame:start()
  self:on_create()
  GAME = self
end

-- Overlay fakes (Phase 5b: the terminal's eggs). test_acid_eggs.lua
-- replaces these with its own counters.
OVERLAY_OK = true
function acid_overlay_open() push(CALLS, { "overlay_open" }); return OVERLAY_OK end
function acid_overlay_clear() push(CALLS, { "overlay_clear" }) end
function acid_overlay_close() push(CALLS, { "overlay_close" }) end

-- Assertions: eq/ok/group, shared by every test file.
local function show(v)
  if type(v) ~= "table" then return tostring(v) end
  local parts = {}
  for i = 1, #v do parts[#parts + 1] = show(v[i]) end
  for k, x in pairs(v) do
    if type(k) ~= "number" or k < 1 or k > #v or k % 1 ~= 0 then
      parts[#parts + 1] = tostring(k) .. "=" .. show(x)
    end
  end
  return "{" .. table.concat(parts, ", ") .. "}"
end

local function deep_eq(a, b)
  if type(a) ~= "table" or type(b) ~= "table" then return a == b end
  for k, v in pairs(a) do
    if not deep_eq(v, b[k]) then return false end
  end
  for k in pairs(b) do
    if a[k] == nil then return false end
  end
  return true
end

function eq(actual, expected, what)
  if deep_eq(actual, expected) then
    PASSES = PASSES + 1
    print("  ok  " .. what)
  else
    FAILS[#FAILS + 1] = what .. ": expected " .. show(expected) .. ", got " .. show(actual)
    print("FAIL  " .. what)
  end
end

function ok(cond, what)
  eq(not not cond, true, what)
end

function group(name)
  print(name)
end

-- Resizes the app under test the way the kernel would: the live window
-- size changes, then on_resize, then a redraw, with TEXT_AT/RECTS emptied
-- first so a fit check sees only the new frame.
function resize_app(w, h)
  WIN_W, WIN_H = w, h
  GAME:on_resize(w, h)
  TEXT_AT, RECTS, MESH_DRAWS = {}, {}, {}
  GAME:redraw()
end
