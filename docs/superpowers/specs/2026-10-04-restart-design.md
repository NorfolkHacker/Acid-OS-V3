# Restart from Config — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

Config gets a **RESTART** button that brings Acid OS back to the boot-time
screen-size picker without quitting the program by hand.

## Decisions

| Question | Choice |
|---|---|
| Mechanism | A new system call `restart` reaches the platform. The hosted build replaces itself (exec) with a fresh copy of the program, with the same arguments minus `--screen`, on Unix; elsewhere it spawns a copy then exits. A hardware build would reboot, and the default does nothing. There is no in-process teardown. |
| Who may call it | Built-in apps only. A cart is refused, as `close_window` is (spec §16.2). |
| Confirmation | Two presses. The first turns the button into `SURE? PRESS AGAIN`. A second press within 3 seconds restarts. Waiting 3 seconds, or tapping elsewhere, cancels. |
| Wasm | No new import. Carts may not restart, so an import that always refuses would be noise. |

## Out of scope

- Restarting without a new process, by tearing down the kernel, apps, router
  and audio in place.
- Saving open documents, or warning about them. The two-press confirmation
  is the guard. Editor's and Sprite Paint's own unsaved-change checks are
  not consulted.
- A Terminal `restart` command.
- Reboot on hardware. The default `Platform::restart` returns false.

## 1. Platform (`acid-platform`)

`Platform` gains a method with a default body:

```rust
/// Restart the whole OS from its boot screen. Returns false if this
/// platform can't (the default); on success it doesn't return.
fn restart(&self) -> bool { false }
```

## 2. Hosted (`acid-hosted`)

- **`restart_args(args) -> Vec<String>`:** a pure function that drops
  `--screen <v>` and `--screen=<v>` and keeps everything else in order,
  including `--app <name>`. It has its own unit tests.
- **`HostedPlatform::restart`:**
  - it runs `std::env::current_exe()` with
    `restart_args(std::env::args().skip(1))`, using the current working
    directory and inherited stdio;
  - on Unix it replaces the process (`exec`), so the copy keeps the same
    process id, process group and terminal: Ctrl+C and the launching
    shell's wait carry over, and every thread and the old window go with
    the old image;
  - elsewhere it spawns the copy and calls `std::process::exit(0)`, which
    closes the old window;
  - if `current_exe` or the exec or spawn fails, it prints the error to stderr and
    returns false.

## 3. Kernel and API (`acid-kernel`, `acid-api`, `acid-lua`)

- `Kernel::restart(&self) -> bool` calls `self.platform().restart()`.
- `AcidApi` gains `fn restart(&self) -> bool`, a required method so that
  every implementor decides.
  - `KernelApi` returns false for a cart, and otherwise returns
    `kernel.restart()`.
  - The test fakes return false and log nothing.
- The Lua binding is `acid_restart()`, which returns a bool.
- `FakePlatform` (`acid-testkit`) records restart calls in a counter and
  returns true, so kernel and API tests can see a call reach the platform.

## 4. Config (`apps/config.lua`)

- **Window:** it grows from 180×190 to 180×242, and `config.app.toml`
  changes with it. Existing coordinates are unchanged.
- **New section:** below the font note, which stays at y 180:
  - a `SYSTEM` label at y 196;
  - a full-width button at `(BAR_X, 212, BAR_W, 20)`.
- **Button states:**
  - normally it reads `RESTART`, in panel colours;
  - once armed, it reads `SURE? PRESS AGAIN` on `THEME_VIOLET` (`0xB026FF`)
    with background-coloured text;
  - if `acid_restart()` returns false, it reads `RESTART FAILED` in muted
    text until the next press.
- **Arming:**
  - a press on the button while it isn't armed arms it, records
    `acid_now_ms()`, and redraws;
  - a press while armed calls `acid_restart()`.
- **Cancelling:**
  - `on_idle` disarms the button once 3000 ms have passed since arming;
  - any press elsewhere in the window disarms it.
- **Repeated touches:** the existing press-once-per-hold guard applies. A
  held press can't arm and confirm in one hold.

The file's header comment, which lists Config's knobs, gains Restart.

## 5. Testing

- **Platform:** the default `restart` returns false.
- **Hosted:** `restart_args` cases:
  - `[] → []`;
  - `--screen 800x600` is dropped;
  - `--screen=640x360` is dropped;
  - `--app tetris --screen 800x600` keeps `--app tetris`;
  - a trailing `--screen` with no value is dropped;
  - unrelated arguments keep their order.
- **API:**
  - a built-in app's `restart()` reaches the platform: the fake counter is
    1 and it returns true;
  - a cart's `restart()` returns false and the counter stays 0.
- **Lua:** `acid_restart()` returns the API's result.
- **Config suite** (`tools/test_config.lua`, extended):
  - the first press arms the button and shows the label, without calling
    restart;
  - a second press calls `acid_restart` once;
  - holding a press calls it at most once and never arms then confirms
    within a single hold;
  - 3 seconds of idle disarms it;
  - a press elsewhere disarms it;
  - a refused restart shows `RESTART FAILED`;
  - everything drawn fits the 180×242 window.
- **Manual check by the user:** open Config, press RESTART twice, and the
  OS should come back at the size picker in a new window.

## 6. Docs

- `01-getting-started.md`: one line about Config → RESTART.
- `09-api-reference.md`: `acid_restart()`, built-in apps only, with an
  index entry.
- `07-system-apis.md`, in the section on what a cart is refused: add
  `acid_restart`.
- `10-wasm-carts.md`: no change, since there's no import.

## Risks

- **Losing unsaved work.** Restart ends every app at once. The two-press
  confirmation is the only guard, and that is accepted in the decisions
  above.
- **Starting fails.** The old copy keeps running and Config says so.
- **Half-written files.** Apps that write files directly (Sprite Paint,
  Load Cart) could leave a truncated file if a restart lands mid-write.
  That is accepted, and the two-press confirmation covers it.
