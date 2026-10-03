# The Acid OS v3 Manual

Acid OS v3 is a small graphical operating system written in Rust. This build
is the **hosted x64** one: it runs in a window on your desktop. Every
application is a **Lua** script running in its own Lua VM, on its own OS
thread, in its own window. Applications can also be **WASM carts**: a
`.wasm` module compiled from Rust (or anything else that targets WebAssembly),
run by the same kernel behind a stricter sandbox.

This manual is about writing those applications. Every Lua and WAT example
in it is tested on each `cargo test`: the Lua apps and the WAT cart run under
the real kernel, and every Lua snippet is parsed. The Rust cart excerpts are
not compiled, but the sample cart in `v3/carts-src/hello-wasm` is built from
source and run by the tests.

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

That is a complete, installable app. Drop it in `v3/apps/hello.lua` with a
manifest beside it and it appears in the Menu at the next boot: no rebuild, no
Rust, no toolchain.

## Contents

| | |
|---|---|
| **[1. Getting started](01-getting-started.md)** | Build the hosted OS, run it, write and install your first app. |
| **[2. Apps and manifests](02-apps-and-manifests.md)** | `.app.toml`, how the launcher finds apps, and the `.cart` format for apps written outside the OS. |
| **[3. The app lifecycle](03-app-lifecycle.md)** | `AcidApp`, the event loop, touch, keys, idle, redraw, focus. |
| **[4. Graphics](04-graphics.md)** | Drawing inside your window, the theme palette, the 256-colour hue wheel, the full-screen overlay and sprites. |
| **[5. Sound](05-sound.md)** | The 8-voice synthesiser: notes, envelopes, waveforms, the filter, ring modulation and the arpeggiator. |
| **[6. Games](06-games.md)** | `AcidGame`, fixed-tick loops, and the sound-effect lifecycle. |
| **[7. System APIs](07-system-apis.md)** | Windows, launching other apps, task and memory stats, network, master volume. |
| **[8. Cookbook](08-cookbook.md)** | Recipes, conventions and the mistakes that bite. |
| **[9. API reference](09-api-reference.md)** | Every `acid_*` function and every library module, alphabetically. |
| **[10. WASM carts](10-wasm-carts.md)** | Writing an app as a WebAssembly module: the callback ABI, the import table, limits and error codes. |
