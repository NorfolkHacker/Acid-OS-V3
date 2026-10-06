# Folders and a Source Lock — Design

**Date:** 2026-10-07
**Status:** approved in brainstorming, awaiting spec review

## Goal

Make files easier to find, and protect the OS's own code from accidental
damage.

1. **The File Manager's top level** becomes: `Apps`, `Games`, `Source`,
   `Home`, `Help`, `Tmp`.
   - `Apps` and `Games` list launchers by display name. Selecting one
     launches it.
   - `Source` holds all the system code.
2. **System source is read-only from inside the OS** unless **Developer
   Mode** is on. Developer Mode is a switch in Config, and it is off at
   every boot.

## Decisions

| Question | Choice |
|---|---|
| Protection | Read-only system source, plus a Developer Mode unlock in Config. It isn't saved: off at every boot. |
| Where it's enforced | `acid-api`'s write, rename and delete calls. That covers every app: Lua, WASM, built-in apps and carts. |
| What counts as system source | Every path under `v3/apps`, spelt `v3/apps/…` or `v3/fsroot/Source/…`. |
| Top level | Apps, then Games (both virtual, always first), then the real folders Source, Home, Help and Tmp. |
| How Apps and Games are built | The File Manager reads `v3/apps/*.app.toml` and sorts each manifest by a new `category = app\|game` field. A missing field means `app`. Each entry is listed by its `name`. |
| The old links | `fsroot/App` is renamed `fsroot/Source`. `fsroot/Lib` is removed; library code is at `Source/lib`. No compatibility link is kept. |
| Unchanged | The desktop Menu (still driven by `menu = false`), Terminal's `run`, and cart rules (carts still change only `Home`). |

## Out of scope

- More categories (Toys, Tools) and user-defined folders.
- Saving Developer Mode across boots.
- Protecting anything outside `v3/apps`. Home is the user's own, and Help and Tmp stay writable.
- Making Terminal's `ls /` show the virtual Apps and Games folders.

## 1. The source lock (`acid-api`, `acid-kernel`)

### 1.1 Developer Mode flag

- The kernel gains `dev_mode: AtomicBool`, starting `false`, with `dev_mode()` and `set_dev_mode(bool)`. It lives in memory only, like the font setting.
- `AcidApi` gains `dev_mode() -> bool` and `set_dev_mode(on: bool) -> bool`.
  - For a cart, `set_dev_mode` returns `false` and changes nothing, as the existing restart rule does.
  - For a built-in app it sets the flag and returns `true`.
- Lua gets `acid_get_dev_mode()` and `acid_set_dev_mode(on)`.
- WASM carts get neither call yet.

### 1.2 The lock

A path is **system source** if it is `v3/apps` or starts with `v3/apps/`, or is `v3/fsroot/Source` or starts with `v3/fsroot/Source/`. This is checked after `fs_path_is_allowed`, on the path string as given.

While Developer Mode is off:
- `fs_write(path, …)` on system source returns `Err("read only")`.
- `fs_rename(from, to)` returns `Err("read only")` if either end is system source.
- `fs_delete(path)` on system source returns `Err("read only")`.

Reading, listing, sizing and launching are never locked. With Developer Mode on, these calls behave as they do today. The existing cart rule (carts change only under Home) still applies on top.

## 2. Config: the DEV MODE row

- Config gains a row labelled `DEV MODE` with an OFF/ON toggle. It sits below FONT and above RESTART, and follows the existing button style.
- `ON` is drawn in the hard theme colour, so it's obvious while it's on. The note under it reads `system source is writable`.
- Pressing the toggle calls `acid_set_dev_mode`.
- Config's window grows as needed to fit. Its manifest `h` is updated, and everything still fits at every screen size.

## 3. File Manager

### 3.1 The top level

At `v3/fsroot` the list is:
1. `Apps/` and `Games/`, which are virtual;
2. the real directory entries, as today. After the link rename these are `Help`, `Home`, `Source` and `Tmp`, and they keep the File Manager's existing order.

### 3.2 The Apps and Games views

- Entering either one lists `..`, then one entry per `v3/apps/*.app.toml` whose `category` matches (a missing field means `app`). Each entry is labelled with its `name` field, sorted case-insensitively.
- Selecting an entry launches it through the existing `launch_manifest` path.
- The header row shows `Apps: <desc>` (or `Games: …`) for the selected entry.
- `..` returns to `v3/fsroot`.
- Delete and rename do nothing in these views.
- The File Manager knows it's in a virtual view through its own state (`self.view = "apps" | "games" | nil`). `self.dir` stays `v3/fsroot` while in a view.

### 3.3 Messages

File Manager has no delete or rename, so the lock's user-facing message is the Editor's (§4).

## 4. Editor

When saving returns `read only`, the Editor shows `read only: save to Home, or DEV MODE in Config` instead of `save failed: read only`. Opening and reading system files are unchanged.

## 5. Manifests

Every manifest in `v3/apps` gains `category`:
- **game:** tetris, breakout, acid_blaster, acidstorm, acid_snake, acid_invaders, acid_rocks.
- **app:** every other manifest.

## 6. Renaming the links

- `v3/fsroot/App` (→ `../apps`) is renamed `v3/fsroot/Source`.
- `v3/fsroot/Lib` is deleted.
- Every reference to `fsroot/App` or `fsroot/Lib` in code, tests and docs is updated to `fsroot/Source`. That includes:
  - `AcidApp.FSROOT_APP_PREFIX` and the File Manager's and Editor's canonical-path mapping;
  - `layout.rs`, `std_impl.rs`, `window.rs` and `lua_app.rs`;
  - the Lua suites;
  - manual chapters 1, 2, 7, 9 and 11;
  - the README.

## 7. Testing

- **`acid-api`, the lock:**
  - with Developer Mode off, write, rename (each end) and delete under both spellings return `read only`;
  - with it on, they succeed;
  - Home is unaffected either way;
  - a cart's `set_dev_mode(true)` returns false and leaves the flag off.
- **Kernel:** the flag is off on a new kernel.
- **Config (Lua suite):**
  - the toggle calls `acid_set_dev_mode` (faked in `game_test_env.lua`);
  - it draws ON in the hard colour;
  - it fits the window.
- **File Manager (Lua suite):**
  - the top level lists Apps and Games first;
  - each view lists display names for its category only, with a missing category treated as `app`;
  - selecting one launches it, with the right spawn call;
  - `..` returns to the top level;
  - delete and rename do nothing in a view;
  - the `read only` message shows.
- **Editor (Lua suite):** a `read only` save shows the hint.
- **OS tests:**
  - the `layout.rs` link test expects `Source → ../apps`, and expects no `App` or `Lib`;
  - `games.rs` and `golden.rs` paths are updated;
  - the File Manager goldens are made again after viewing them;
  - the Menu and desktop goldens must not change.
- **Manual tests:** every link resolves, including the updated paths.
