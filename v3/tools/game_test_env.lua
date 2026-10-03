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
DRAW_CALLS = 0
CLOCK = 0
FAILS = {}
PASSES = 0

local function push(t, v) t[#t + 1] = v end

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
function acid_draw_text(str, x, y, fg, bg) DRAW_CALLS = DRAW_CALLS + 1; push(TEXTS, str) end
function acid_fill_circle(x, y, r, color) DRAW_CALLS = DRAW_CALLS + 1 end
function acid_fill_rect(x, y, w, h, color)
  DRAW_CALLS = DRAW_CALLS + 1
  push(RECTS, { x, y, w, h, color })
end
function acid_overlay_fill_rect(x, y, w, h, color) push(RECTS, { x, y, w, h, color }) end
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
