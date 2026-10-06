# Folders and a Source Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the OS's own source read-only unless Developer Mode is on, and give the File Manager a top level of Apps, Games, Source, Home, Help and Tmp.

**Architecture:**
- **The lock.** It lives in `acid-api`'s write, rename and delete calls, so every app is covered. A kernel `AtomicBool` holds Developer Mode. It's off at boot, and Config switches it.
- **Apps and Games are virtual views.** The File Manager builds them from `v3/apps/*.app.toml` and a new `category` field.
- **The links.** `fsroot/App` is renamed `fsroot/Source`, and `fsroot/Lib` is removed.

**Tech Stack:** Rust (`acid-kernel`, `acid-api`, `acid-lua`), Lua apps (Config, File Manager, Editor), the Lua suite runner, and OS golden tests.

**Spec:** `docs/superpowers/specs/2026-10-07-folders-and-source-lock-design.md`

## Global Constraints

- **Running commands.** Run every command from the repository top (`/home/norfolkh/acid-os-v3`), using `--manifest-path v3/Cargo.toml`.
- **Commits.** Messages use the style `Area: summary`, and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **What's system source:** any path equal to, or under, `v3/apps` or `v3/fsroot/Source`.
- **The lock message** is exactly `read only`, which is the existing cart message.
- **Developer Mode** is off at every boot and is never saved. A cart can't change it.
- **Lua assertion counts** are pinned in `v3/crates/acid-lua/tests/game_tests.rs`. Update a count only when the assertions in its suite change, and say why.
- **Golden images.** Never remake one to make a test pass. A golden may be remade only for an intended visual change, after viewing the frame as a PNG.
- **Overflow limits (standing user rule).** Clamp or range-check any number that comes from input. (There are few numbers in this plan.)

## Corrections to the spec, decided while planning

- **No delete or rename in File Manager.** File Manager doesn't have them today, so spec §3.3 (its `read only` message) has nothing to apply to. The lock's user-facing message is the Editor's (§4). Task 4 updates the spec.
- **No status line in File Manager either.** The selected launcher's `desc` goes on the header row instead, as `Apps: <desc>`.

## File map

| File | Change |
|---|---|
| `v3/crates/acid-kernel/src/kernel.rs` | `dev_mode` flag |
| `v3/crates/acid-api/src/lib.rs` | `is_system_source`, `may_change`, trait `dev_mode`/`set_dev_mode`, tests |
| `v3/crates/acid-lua/src/lib.rs` | `acid_get_dev_mode`, `acid_set_dev_mode` |
| `v3/tools/game_test_env.lua` | Fakes for the dev-mode calls; `FAIL_WRITES` may name an error |
| `v3/apps/config.lua`, `config.app.toml`, `v3/tools/test_config.lua` | DEV MODE row |
| `v3/apps/editor.lua`, `v3/tools/test_editor_app.lua` | The read-only save message |
| `v3/fsroot/App` → `v3/fsroot/Source`, and `v3/fsroot/Lib` removed | The links |
| Every `fsroot/App` reference in code and tests | Updated to `Source` |
| `v3/apps/*.app.toml` | `category = app` or `game` |
| `v3/apps/file_manager.lua`, `v3/tools/test_file_manager*.lua` | The Apps and Games views |
| `v3/crates/acid-os/tests/golden/file_manager_*.ppm` | Remade after viewing |
| `docs/manual-v3/*.md`, `README.md`, the spec | Docs |

---

### Task 1: Developer Mode and the source lock

**Files:**
- Modify: `v3/crates/acid-kernel/src/kernel.rs`
- Modify: `v3/crates/acid-api/src/lib.rs`
- Modify: `v3/crates/acid-lua/src/lib.rs`
- Modify: `v3/tools/game_test_env.lua`

**Interfaces:**
- Produces:
  - `Kernel::dev_mode() -> bool` and `Kernel::set_dev_mode(bool)`
  - `AcidApi::dev_mode() -> bool` and `AcidApi::set_dev_mode(bool) -> bool`, both with default bodies so the test fakes compile
  - the Lua calls `acid_get_dev_mode()` and `acid_set_dev_mode(on)`, which returns a bool
  - Lua fakes: a `DEV_MODE` global, and a `{ "set_dev_mode", on }` entry pushed to `CALLS`

- [ ] **Step 1: Write the failing `acid-api` tests**

Add these to `mod tests` in `v3/crates/acid-api/src/lib.rs`, after `write_calls_are_path_guarded`:

```rust
    #[test]
    fn system_source_is_read_only_unless_developer_mode_is_on() {
        let (k, api) = spawn_on(FakePlatform::new(FakePlatform::repo_root()), 10, 10);
        let name = format!("v3/apps/lock-test-{}.txt", std::process::id());
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(FakePlatform::repo_root().join(&name));
        let ro = Err(String::from("read only"));
        assert!(!k.dev_mode(), "Developer Mode is off at boot");
        assert_eq!(api.fs_write(&name, b"x"), ro);
        assert_eq!(api.fs_delete("v3/apps/hello_acid.lua"), ro);
        assert_eq!(api.fs_rename("v3/apps/hello_acid.lua", "v3/fsroot/Tmp/x.lua"), ro, "out of source");
        assert_eq!(api.fs_rename("v3/fsroot/Home/notes.txt", &name), ro, "into source");
        assert_eq!(api.fs_write("v3/fsroot/Source/x.lua", b"x"), ro, "the Source spelling is locked too");
        assert!(api.fs_read("v3/apps/hello_acid.app.toml").is_ok(), "reading is never locked");
        assert!(api.set_dev_mode(true));
        assert!(api.dev_mode());
        assert_eq!(api.fs_write(&name, b"x"), Ok(()));
        assert_eq!(api.fs_delete(&name), Ok(()));
        assert!(api.set_dev_mode(false));
        let tmp = format!("v3/fsroot/Tmp/lock-test-{}.txt", std::process::id());
        let _c2 = Cleanup(FakePlatform::repo_root().join(&tmp));
        assert_eq!(api.fs_write(&tmp, b"x"), Ok(()), "outside source nothing changes");
        assert_eq!(api.fs_delete(&tmp), Ok(()));
    }

    #[test]
    fn a_cart_cannot_turn_on_developer_mode() {
        let (k, cart) = spawn_cart_on(FakePlatform::new(FakePlatform::repo_root()), 10, 10);
        assert!(!cart.set_dev_mode(true));
        assert!(!k.dev_mode());
    }
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-api system_source`
Expected: FAIL to compile, with "no method named `dev_mode`".

- [ ] **Step 2: The kernel flag**

In `v3/crates/acid-kernel/src/kernel.rs`, `AtomicBool` is already imported. Make three additions:
1. Add a field to `Kernel` after `wallpaper_enabled: AtomicBool,`:

```rust
    /// Config's Developer Mode: while on, system source (v3/apps) is
    /// writable. Off at every boot, never saved.
    dev_mode: AtomicBool,
```

2. Initialise it in the constructor, next to `wallpaper_enabled`, as `dev_mode: AtomicBool::new(false),`.
3. Add these after `set_font_scale`:

```rust
    pub fn dev_mode(&self) -> bool {
        self.dev_mode.load(Ordering::SeqCst)
    }

    pub fn set_dev_mode(&self, on: bool) {
        self.dev_mode.store(on, Ordering::SeqCst);
    }
```

- [ ] **Step 3: The lock and the API calls**

In `v3/crates/acid-api/src/lib.rs`:

1. Add this free function next to `fs_error`:

```rust
/// The OS's own source: v3/apps, reached directly or through fsroot's
/// Source link. Read-only unless Developer Mode is on.
fn is_system_source(path: &str) -> bool {
    ["v3/apps", "v3/fsroot/Source"]
        .iter()
        .any(|r| path == *r || path.strip_prefix(r).is_some_and(|rest| rest.starts_with('/')))
}
```

2. Add this to `impl KernelApi`, after `cart_may_change`:

```rust
    /// Whether the caller may change `path`: carts only under Home, and
    /// nobody touches system source while Developer Mode is off.
    fn may_change(&self, path: &str) -> bool {
        self.cart_may_change(path) && (!is_system_source(path) || self.ctx.kernel.dev_mode())
    }
```

3. In `fs_write`, `fs_rename` (both ends) and `fs_delete`, replace each `self.cart_may_change(` with `self.may_change(`.

4. Add these to `trait AcidApi`, after `fn restart(&self) -> bool;`:

```rust
    /// Developer Mode: while on, system source is writable. Off at boot.
    fn dev_mode(&self) -> bool { false }
    /// Sets Developer Mode; false (and no change) for a cart.
    fn set_dev_mode(&self, _on: bool) -> bool { false }
```

5. Add these to `impl AcidApi for KernelApi`, after `restart`:

```rust
    fn dev_mode(&self) -> bool {
        self.ctx.kernel.dev_mode()
    }

    fn set_dev_mode(&self, on: bool) -> bool {
        // Unlocking the OS's own code is not a cart's call (spec §16.2).
        if self.ctx.cart {
            return false;
        }
        self.ctx.kernel.set_dev_mode(on);
        true
    }
```

- [ ] **Step 4: Lua bindings and fakes**

In `v3/crates/acid-lua/src/lib.rs`, add these after the `acid_restart` binding:

```rust
    let a = api.clone();
    g.set("acid_get_dev_mode", lua.create_function(move |_, ()| Ok(a.dev_mode()))?)?;
    let a = api.clone();
    g.set("acid_set_dev_mode", lua.create_function(move |_, on: bool| Ok(a.set_dev_mode(on)))?)?;
```

In `v3/tools/game_test_env.lua`, make two changes:
1. Add these after `function acid_restart() … end`:

```lua
DEV_MODE = false   -- what acid_get_dev_mode answers
function acid_get_dev_mode() return DEV_MODE end
function acid_set_dev_mode(on) push(CALLS, { "set_dev_mode", on }); DEV_MODE = on; return true end
```

2. Change the failing branch of `acid_fs_write`, so a test can choose the error:

```lua
  if FAIL_WRITES[path] then return nil, type(FAIL_WRITES[path]) == "string" and FAIL_WRITES[path] or "disk full" end
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-kernel -p acid-api -p acid-lua`
Expected: all pass, including the two new tests. No Lua suite count changes.

- [ ] **Step 6: Commit**

```bash
git add v3/crates/acid-kernel/src/kernel.rs v3/crates/acid-api/src/lib.rs v3/crates/acid-lua/src/lib.rs v3/tools/game_test_env.lua
git commit -m "Lock: system source is read-only unless Developer Mode is on

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Config's DEV MODE row

**Files:**
- Modify: `v3/apps/config.lua`
- Modify: `v3/apps/config.app.toml`
- Modify: `v3/tools/test_config.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`, changing the `config` count from 32 to 37

**Interfaces:**
- Consumes: `acid_get_dev_mode` and `acid_set_dev_mode` (Task 1)
- Produces: a DEV MODE toggle button at y 212..232, with a note at y 236. SYSTEM/RESTART moves to 252/268..288, and the window is 180×298.

- [ ] **Step 1: Write the failing tests**

In `v3/tools/test_config.lua`, the `restart` group first:
- Replace every `tap(90, 220)` with `tap(90, 278)`.
- Replace `GAME:on_touch(90, 220, true)` with `GAME:on_touch(90, 278, true)`.
- Replace `WIN_W, WIN_H = 180, 242` with `WIN_W, WIN_H = 180, 298`.
- Change the fit message to `"everything fits the 180x298 window"`.

Then append:

```lua
group("developer mode")
DEV_MODE = false
GAME:on_create()
eq(GAME.dev_mode, false, "Developer Mode is read back from the kernel")
TEXTS = {}
GAME:redraw()
ok(has(TEXTS, "DEV MODE") and has(TEXTS, "system source is read-only"), "a DEV MODE row says source is read-only")
CALLS, TEXTS = {}, {}
tap(90, 220)
eq({ CALLS[1], GAME.dev_mode }, { { "set_dev_mode", true }, true }, "tapping it turns Developer Mode on")
ok(has(TEXTS, "system source is writable"), "and then says source is writable")
tap(90, 220)
eq({ CALLS[2], DEV_MODE }, { { "set_dev_mode", false }, false }, "tapping again turns it off")
```

Change `config`'s count in `game_tests.rs` to 37.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests config`
Expected: FAIL.

- [ ] **Step 2: Implement it**

In `v3/apps/config.lua`:

1. Change the header comment's first line to `-- Config -- system-wide settings.`, and list Developer Mode with the real knobs.

2. Change `ConfigApp.WINDOW_H = 242` to `ConfigApp.WINDOW_H = 298`.

3. Replace the RESTART constants block with:

```lua
-- DEV MODE, below the font note (button 212..232): while on, the OS's own
-- source (Source, v3/apps) is writable. Off at every boot.
ConfigApp.DEV_LABEL_Y = 196
ConfigApp.DEV_BTN_Y = 212
ConfigApp.DEV_BTN_H = 20
ConfigApp.DEV_NOTE_Y = 236

-- RESTART, below DEV MODE (button 268..288). It takes two presses:
-- the first arms it, a second within RESTART_CONFIRM_MS restarts.
ConfigApp.SYSTEM_LABEL_Y = 252
ConfigApp.RESTART_BTN_Y = 268
ConfigApp.RESTART_BTN_H = 20
ConfigApp.RESTART_CONFIRM_MS = 3000
```

4. In `on_create`, add `self.dev_mode = acid_get_dev_mode()`.

5. In `redraw`, add these before the SYSTEM label:

```lua
  acid_draw_text("DEV MODE", 4, C.DEV_LABEL_Y, C.MUTED_COLOR, C.BG_COLOR)
  self:draw_dev()
```

6. Add these methods after `draw_toggle`:

```lua
function ConfigApp:draw_dev()
  local C = ConfigApp
  local label = self.dev_mode and "ON" or "OFF"
  local bg = self.dev_mode and C.HARD_COLOR or C.PANEL_COLOR
  local fg = self.dev_mode and C.BG_COLOR or C.TEXT_COLOR
  acid_fill_rect(C.BAR_X, C.DEV_BTN_Y, C.BAR_W, C.DEV_BTN_H, bg)
  acid_draw_text(label, C.BAR_X + C.BAR_W // 2 - #label * 3, C.DEV_BTN_Y + 6, fg, bg)
  local note = self.dev_mode and "system source is writable" or "system source is read-only"
  acid_draw_text(note, 4, C.DEV_NOTE_Y, self.dev_mode and C.HARD_COLOR or C.MUTED_COLOR, C.BG_COLOR)
end

function ConfigApp:set_dev(on)
  if acid_set_dev_mode(on) then self.dev_mode = on end
  self:redraw()
end
```

7. In `on_touch`, add this after the font-button block, at the end of the function:

```lua
  if y >= C.DEV_BTN_Y and y < C.DEV_BTN_Y + C.DEV_BTN_H
      and x >= C.BAR_X and x < C.BAR_X + C.BAR_W then
    self:set_dev(not self.dev_mode)
  end
```

In `v3/apps/config.app.toml`, change `h = 242` to `h = 298`.

- [ ] **Step 3: Run the tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests config`
Expected: PASS, with 37 assertions.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test games config`
Expected: PASS. Config still boots.

- [ ] **Step 4: Commit**

```bash
git add v3/apps/config.lua v3/apps/config.app.toml v3/tools/test_config.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Config: a DEV MODE toggle that unlocks system source

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The Editor explains a locked save

**Files:**
- Modify: `v3/apps/editor.lua`
- Modify: `v3/tools/test_editor_app.lua`
- Modify: `game_tests.rs`, changing the `editor_app` count from 26 to 27

- [ ] **Step 1: Write the failing test**

In `v3/tools/test_editor_app.lua`, insert this directly after the line `FAIL_RENAMES = {}` that ends the failed-save group:

```lua
FAIL_WRITES[tmp] = "read only"
esc("s")
eq(G.message, "read only: save as to Home, or turn on DEV MODE in Config", "a locked save says how to get round it")
FAIL_WRITES = {}
```

Change `editor_app`'s count to 27.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests editor_app`
Expected: FAIL, because the message is `save failed`.

- [ ] **Step 2: Implement it**

In `EditorApp:save_file` in `v3/apps/editor.lua`, keep the write's error:

```lua
  local wrote, werr = acid_fs_write(tmp, table.concat(self.buf:lines(), "\n") .. "\n")
  wrote = wrote ~= nil
```

Replace `self.message = "save failed"` with:

```lua
    -- System source is locked unless Developer Mode is on (Config).
    self.message = werr == "read only" and "read only: save as to Home, or turn on DEV MODE in Config"
      or "save failed"
```

- [ ] **Step 3: Run the Editor suites**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests editor`
Expected: all pass. The editor_app suite has 27 assertions.

- [ ] **Step 4: Commit**

```bash
git add v3/apps/editor.lua v3/tools/test_editor_app.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Editor: a locked save says how to get round it

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Source replaces App and Lib; Apps and Games views in File Manager

**Files:**
- Move: `v3/fsroot/App` → `v3/fsroot/Source`. Delete: `v3/fsroot/Lib`.
- Modify every `fsroot/App` reference in code and tests:
  - `v3/apps/lib/acid_app.lua` (`FSROOT_APP_PREFIX`)
  - `v3/tools/game_test_env.lua` (`FSROOT_APP_PREFIX`)
  - `v3/apps/editor/layout.lua` (`OWN_SOURCE_ROOTS` and its comment)
  - the comments in `v3/apps/editor.lua`, `v3/apps/editor/cmdbar.lua` and `v3/apps/file_manager.lua`
  - `v3/tools/test_editor_app.lua`, `test_scroll_editor.lua`, `test_file_manager.lua`
  - `v3/crates/acid-lua/tests/lua_app.rs`
  - `v3/crates/acid-os/tests/games.rs` and `layout.rs`
  - `v3/crates/acid-kernel/src/fs_path.rs` and `window.rs` (test strings)
  - `v3/crates/acid-platform/src/std_impl.rs` (its tests make their own link: rename it to `Source`)
- Modify: every `v3/apps/*.app.toml`, adding `category`
- Modify: `v3/apps/file_manager.lua`, `v3/tools/test_file_manager.lua`
- Remake: `v3/crates/acid-os/tests/golden/file_manager_large.ppm` and `file_manager_resized.ppm`, after viewing
- Modify: `docs/superpowers/specs/2026-10-07-folders-and-source-lock-design.md` (§3.3 and §3.2)

**Interfaces:**
- **The top level, `v3/fsroot`,** lists two virtual entries first, then the real folders:
  - `{ name = "Apps", dir = true, view = "apps", size = 0 }`
  - `{ name = "Games", dir = true, view = "games", size = 0 }`
- **In a view** (`self.view` is `"apps"` or `"games"`), the entries are:
  - first, `{ name = "..", dir = true, size = 0 }`
  - then one `{ name = <manifest file>, label = <name>, desc = <desc>, launch = true, dir = false, size = 0 }` for each matching manifest, sorted by `label:lower()`

- [ ] **Step 1: Rename the links, and update the references**

Run:

```bash
git mv v3/fsroot/App v3/fsroot/Source
git rm v3/fsroot/Lib
```

Then replace `fsroot/App` with `fsroot/Source` in every file listed above. Comments that describe the link say "Source" from now on. Check that nothing is left:

```bash
grep -rn 'fsroot/App\|fsroot/Lib' v3/apps v3/tools v3/crates --include='*.lua' --include='*.rs'
```

Expected: no matches.

In `v3/crates/acid-os/tests/layout.rs`:
- The link test must expect exactly `("v3/fsroot/Source", "../apps")`, and must check that `v3/fsroot/App` and `v3/fsroot/Lib` don't exist.
- Rename `the_libs_are_browsable_through_lib` to `the_libs_are_browsable_through_source`, listing `v3/fsroot/Source/lib`.

In `v3/tools/test_file_manager.lua`, rename the fake folder `App` to `Source` in the FS table and in the expectations. The root then sorts as `Help, Many, Source, long.txt, readme.txt`.
- Adjust only the cursor movement (the number of `key(AcidKeys.DOWN)` presses) and the index-based expectations to the new order, keeping each assertion's meaning.
- The assertion count must not change.
- Step 3 will shift the root again by two entries, so you may do both adjustments together at the end of Step 3. Commit the rename only once all suites pass.

- [ ] **Step 2: Categories in the manifests**

Add `category = game` to `tetris`, `breakout`, `acid_blaster`, `acidstorm`, `acid_snake`, `acid_invaders` and `acid_rocks` (`.app.toml`). Add `category = app` to every other `v3/apps/*.app.toml`. Put the line directly after `desc`.

- [ ] **Step 3: Write the failing File Manager tests**

Append to `v3/tools/test_file_manager.lua`:

```lua
group("Apps and Games")
FS["v3/fsroot"] = { "Home", "Source" }
FS["v3/fsroot/Home"] = {}
FS["v3/fsroot/Source"] = {}
FS["v3/apps"] = { "zed.app.toml", "alpha.app.toml", "tetris.app.toml", "old.app.toml", "tetris.lua" }
FS["v3/apps/zed.app.toml"] = "name = Zed\nw = 100\nh = 80\ndesc = last app\ncategory = app\n"
FS["v3/apps/alpha.app.toml"] = "name = alpha\nw = 100\nh = 80\ndesc = first app\ncategory = app\n"
FS["v3/apps/tetris.app.toml"] = "name = Tetris\nw = 160\nh = 160\ndesc = blocks\ncategory = game\n"
FS["v3/apps/old.app.toml"] = "name = Old\nw = 90\nh = 70\n"
G:on_create()
eq(names(), { "Apps", "Games", "Home", "Source" }, "Apps and Games come first at the top level")
key(AcidKeys.ENTER)
local labels = {}
for i, e in ipairs(G.entries) do labels[i] = e.label or e.name end
eq(labels, { "..", "alpha", "Old", "Zed" }, "Apps lists app names, a missing category counting as app")
key(AcidKeys.DOWN)
TEXTS = {}
G:redraw()
ok(has(TEXTS, "Apps: first app"), "the header shows the selected app's description")
CALLS = {}
key(AcidKeys.ENTER)
eq(CALLS[1], { "spawn", "v3/apps/alpha.lua", 100, 80, "" }, "Enter launches the app")
key(AcidKeys.BACKSPACE)
eq({ G.view, names()[1] }, { nil, "Apps" }, "Backspace goes back to the top level")
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
labels = {}
for i, e in ipairs(G.entries) do labels[i] = e.label or e.name end
eq(labels, { "..", "Tetris" }, "Games lists only games")
key(AcidKeys.ENTER)
eq(G.view, nil, "the .. row goes back up too")
```

That is 7 new assertions. Change `file_manager`'s count in `game_tests.rs` from 36 to 43. Then run the suite:

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests file_manager`
Expected: FAIL.

The earlier groups' root expectations also gain `Apps` and `Games` at the top. Shift those groups' root cursor moves by two more `DOWN`s, as in Step 1, keeping their meaning. The `test_file_manager_large.lua` and `test_resize_file_manager.lua` suites start in subfolders or don't index the root. If they do index it, give them the same adjustment, with no change to their counts.

- [ ] **Step 4: Implement the views**

In `v3/apps/file_manager.lua`:

1. Update the header comment: the top level offers Apps and Games, which are views built from the manifests, and `Source` is the OS's code.

2. Add this constant after `ROOT_DIR`:

```lua
FileManagerApp.APPS_DIR = "v3/apps"
-- The virtual folders at the top level: { label, view, the category it lists }.
FileManagerApp.VIEWS = { { "Apps", "apps", "app" }, { "Games", "games", "game" } }
```

3. Pull the manifest parsing out of `launch_manifest` into a helper, and have `launch_manifest` take a directory:

```lua
-- A manifest's "key = value" fields, or nil if it can't be read.
function FileManagerApp:read_manifest(path)
  local text = acid_fs_read(path)
  if not text then return nil end
  local fields = {}
  for _, line in ipairs(split_lines(text)) do
    line = trim(line)
    if line ~= "" and line:sub(1, 1) ~= "#" then
      local eq = line:find("=", 1, true)
      if eq then fields[trim(line:sub(1, eq - 1))] = trim(line:sub(eq + 1)) end
    end
  end
  return fields
end
```

`launch_manifest(name, dir)` uses `dir or self.dir` where it used `self.dir`, and calls `self:read_manifest` inside its existing `pcall`, erroring on nil as before.

4. In `on_create`, add `self.view = nil`.

5. At the top of `scan_dir`, after the `..` row logic, add this. When `self.dir == self.ROOT_DIR` and `self.view == nil`, push the two virtual entries before the real ones:

```lua
  if self.dir == self.ROOT_DIR then
    for _, v in ipairs(self.VIEWS) do
      self.entries[#self.entries + 1] = { name = v[1], dir = true, view = v[2], size = 0 }
    end
  end
```

The real entries are still sorted and appended after them. To keep this simple, build `found` as today and append it after the views.

6. Add this:

```lua
-- Apps or Games: one entry per manifest of that category (none means app),
-- labelled with its name. Selecting one launches it.
function FileManagerApp:scan_view()
  self.entries = { { name = "..", dir = true, size = 0 } }
  self.scroll, self.selected = 0, 0
  local want
  for _, v in ipairs(self.VIEWS) do
    if v[2] == self.view then want = v[3] end
  end
  local found = {}
  for _, name in ipairs(acid_fs_list(self.APPS_DIR) or {}) do
    if ends_with(name, ".app.toml") then
      local f = self:read_manifest(self.APPS_DIR .. "/" .. name)
      local category = f and f.category == "game" and "game" or "app"
      if f and f.name and category == want then
        found[#found + 1] = { name = name, label = f.name, desc = f.desc or "", launch = true, dir = false, size = 0 }
      end
    end
  end
  table.sort(found, function(a, b) return a.label:lower() < b.label:lower() end)
  for _, e in ipairs(found) do self.entries[#self.entries + 1] = e end
end
```

7. Make three changes to `activate_selected`:
   - **First branch:** handle the virtual entries, before the `".."` check:

     ```lua
       if entry.view then
         self.view = entry.view
         self:scan_view()
         self:redraw()
         return
       end
       if entry.launch then
         self:launch_manifest(entry.name, self.APPS_DIR)
         return
       end
     ```

   - **`..`:** in a view, the existing `entry.name == ".."` branch calls `go_up`, so that needs nothing extra.
   - **`go_up`:** start it with:

     ```lua
       if self.view then
         self.view = nil
         self:scan_dir()
         self:redraw()
         return
       end
     ```

8. In `draw_listing`:
   - **The header label:** in a view, use the view's label instead of `self.dir`. When the selected entry has a `desc`, add `": " .. desc`, so it reads `Apps: first app`. It is cut to `HEAD_COLS` as now.
   - **An entry with a `label`** is drawn as `" " .. e.label`, with no size.
9. In `entry_color`, `e.launch` entries use `TOML_COLOR`, the violet that means launchable.

10. Any other code that rescans `self.dir` while in a view must go through `scan_view` instead. That includes resizing, and anything else that calls `scan_dir`. Check with `grep -n scan_dir`.

- [ ] **Step 5: Run the Lua suites and the OS tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua`
Expected: all pass. That includes `file_manager` (43), the large and resize variants, the Editor suites (`OWN_SOURCE_ROOTS` now uses `Source`) and `lua_app` (`canonical_app_path`).

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os -p acid-platform -p acid-kernel`
Expected: all pass, except the two File Manager goldens, which now show the new top level.

- [ ] **Step 6: Remake the two File Manager goldens, after looking at them**

For each failing golden (`file_manager_large`, `file_manager_resized`):
1. The test writes `v3/target/actual-<name>.ppm`.
2. Convert it to PNG with the throwaway script below, writing it to the session scratchpad, `/tmp/claude-1000/-home-norfolkh-acid-os-v3/9c1c57fe-36ac-41e2-8f37-08b3563bc286/scratchpad/`:

```python
import sys, zlib, struct
data = open(sys.argv[1], "rb").read()
magic, size, maxval, px = data.split(b"\n", 3)
w, h = map(int, size.split())
raw = b"".join(b"\x00" + px[y * w * 3:(y + 1) * w * 3] for y in range(h))
def chunk(t, d):
    c = struct.pack(">I", len(d)) + t + d
    return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
open(sys.argv[2], "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
```

3. View the PNG with the Read tool. Approve it only if:
   - the top level reads `[Apps] [Games]`, then `[Help] [Home] [Source] [Tmp]`;
   - nothing else in the frame changed;
   - nothing is clipped.
4. Then copy the PPM over its golden in `v3/crates/acid-os/tests/golden/`.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden`
Expected: all pass. The Menu and desktop goldens must pass unchanged. If either of those changes, stop and report.

- [ ] **Step 7: Update the spec**

In `docs/superpowers/specs/2026-10-07-folders-and-source-lock-design.md`:
- Replace §3.3 with: "File Manager has no delete or rename, so the lock's user-facing message is the Editor's (§4)."
- In §3.2, change "the status line shows the selected entry's `desc`" to "the header row shows `Apps: <desc>` (or `Games: …`) for the selected entry".

- [ ] **Step 8: Run the whole workspace, then commit**

Run: `cargo test --manifest-path v3/Cargo.toml`
Expected: all pass.

```bash
git add -A v3/fsroot v3/apps v3/tools v3/crates docs/superpowers/specs/2026-10-07-folders-and-source-lock-design.md
git commit -m "File Manager: Apps and Games views by category; Source replaces App and Lib

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Docs

**Files:**
- Modify: `docs/manual-v3/01-getting-started.md`, `02-apps-and-manifests.md`, `07-system-apis.md`, `09-api-reference.md`, `11-music.md`
- Modify: `README.md` (the repository root)

- [ ] **Step 1: Find every old path**

Run: `grep -rn 'App/\|fsroot/App\|fsroot/Lib\|`Lib`\|App folder' docs/manual-v3 README.md`

Rewrite each match:

| Old text | New text |
|---|---|
| "open `App/<x>.app.toml` in the File Manager" | "open it from the File Manager's **Apps** folder" (or **Games**) |
| a path into the code | `Source/<x>` |
| `Lib/...` | `Source/lib/...` |

- [ ] **Step 2: Document the new behaviour**

In **`02-apps-and-manifests.md`**, add a `category` row to the manifest table:

| Field | What it does |
|---|---|
| `category` | `app` or `game`. It decides whether the File Manager lists the app under **Apps** or **Games**. Missing means `app`. |

Add a sentence saying this is separate from `menu = false`, which only hides an app from the Menu.

In **`07-system-apis.md`**, add a section "Developer Mode and the source lock" that explains:
- the OS's own code (`Source`, `v3/apps`) is read-only from inside the OS;
- writing, renaming or deleting there returns `"read only"`;
- Config's DEV MODE switch unlocks it until the next boot;
- carts can't change it;
- `acid_get_dev_mode()` and `acid_set_dev_mode(on)`, which returns `false` for a cart.

Use a `lua snippet` fence for the example.

In **`09-api-reference.md`**:
- Add `acid_get_dev_mode` and `acid_set_dev_mode` to the **System** line of "Index by area".
- Add their entries in alphabetical order. Use the house format: a `lua snippet` signature, then one paragraph.
- In the `acid_fs_write`, `acid_fs_rename` and `acid_fs_delete` entries, add the sentence: "Returns `nil, "read only"` for system source (`v3/apps`, `Source`) unless Developer Mode is on."

In **`01-getting-started.md`**:
- The File Manager's top level is now Apps, Games, Source, Home, Help and Tmp.
- Games are in **Games**, and Sprite Paint, Acid Tracker and Piano are in **Apps**.
- Mention Developer Mode in one sentence, with a link to chapter 7.

In the **root `README.md`**, change "click their `.app.toml` in the `App` folder" to "open them from the File Manager's **Apps** or **Games** folder".

- [ ] **Step 3: Run the manual tests and the whole workspace**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test manual`
Expected: all pass. The new names are live globals, and every link and anchor resolves.

Run: `cargo test --manifest-path v3/Cargo.toml`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add docs/manual-v3 README.md
git commit -m "Manual: Apps, Games and Source folders; Developer Mode and the source lock

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Task |
|---|---|
| §1.1 the flag and the calls | 1 |
| §1.2 the lock | 1 (both spellings, both rename ends, reading never locked) |
| §2 Config | 2 |
| §3 File Manager | 4 (§3.3 dropped and the header used instead, as recorded above) |
| §4 Editor | 3 |
| §5 manifests | 4 |
| §6 links | 4 |
| §7 tests | 1 to 4 |
| Docs | 5 |

**Task order.** The lock in Task 1 already checks the `v3/fsroot/Source` spelling. Its test only writes the `Source` path while locked, which can't touch the disk, so it doesn't need the link to exist yet. The link itself arrives in Task 4.

**The assertion counts:**

| Suite | Count |
|---|---|
| `config` | 37 |
| `editor_app` | 27 |
| `file_manager` | 43 |
| All others | Unchanged |
