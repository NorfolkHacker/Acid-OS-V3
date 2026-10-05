# The Acid OS v3 Manual

Acid OS v3 is a small graphical operating system written in Rust. The version
in this repository runs in a window on your Linux desktop (we call this the
**hosted** build).

Apps are written in **Lua**. Each app gets its own window, its own Lua VM and
its own thread, so it runs separately from every other app. You can also write apps as **WASM carts**:
WebAssembly modules compiled from Rust, or from anything else that targets
WebAssembly. They run on the same system, but in a stricter sandbox.

This manual shows you how to write those apps. You can trust the examples,
because the test suite checks them every time it runs:

- the complete Lua apps and the WAT cart are run for real
- every shorter Lua snippet is checked to make sure it parses
- the Rust cart excerpts aren't compiled, but the sample cart they come from
  (`v3/carts-src/hello-wasm`) is built and run

Here is a whole app:

```lua app
local HelloApp = AcidApp:extend("HelloApp")

function HelloApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_fill_rect(10, 30, 100, 40, AcidPalette.hue(90))
  acid_draw_text("HELLO ACID", 14, 45, 0x050607, AcidPalette.hue(90))
  acid_draw_window_border()
end

function HelloApp:on_touch(x, y, pressed)
  -- A note sounds until you stop it: there is no duration argument.
  if pressed then acid_play_note(5, 40, 60) else acid_stop_note(5) end
end

HelloApp:new():start()
```

Save it as `v3/apps/hello.lua`, put a small manifest file beside it, and it
appears in the Menu the next time Acid OS starts. You don't rebuild anything
or touch any Rust.

## Contents

| | |
|---|---|
| **[1. Getting started](01-getting-started.md)** | Build and run Acid OS, then write and install your first app. |
| **[2. Apps and manifests](02-apps-and-manifests.md)** | The `.app.toml` manifest, how the Menu finds apps, and `.cart` files for apps written outside the OS. |
| **[3. The app lifecycle](03-app-lifecycle.md)** | `AcidApp`, the event loop, touch, keys, idle, redraw, focus. |
| **[4. Graphics](04-graphics.md)** | Drawing inside your window, the theme palette, the 256-colour hue wheel, the full-screen overlay and sprites. |
| **[5. Sound](05-sound.md)** | The 8-voice synthesiser: notes, envelopes, waveforms, the filter, ring modulation and the arpeggiator. |
| **[6. Games](06-games.md)** | `AcidGame`, fixed-tick loops, the sound-effect lifecycle, held keys and frames. |
| **[7. System APIs](07-system-apis.md)** | Windows, launching other apps, task and memory stats, network, master volume. |
| **[8. Cookbook](08-cookbook.md)** | Recipes, conventions and the mistakes that bite. |
| **[9. API reference](09-api-reference.md)** | Every `acid_*` function and every library module, alphabetically. |
| **[10. WASM carts](10-wasm-carts.md)** | Writing an app as a WebAssembly module: the callback ABI, the import table, limits and error codes. |
