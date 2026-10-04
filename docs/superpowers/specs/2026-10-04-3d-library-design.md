# 3D Library — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

Apps and carts can draw spinning 3D objects: built-in shapes or their own
meshes, drawn as glowing wireframes, flat-shaded solids, or both. Lines
and triangles also become drawing primitives. A new app, **Acid Spin**,
shows it off.

## Decisions

| Question | Choice |
|---|---|
| Look | Wireframe and flat-shaded solids. No textures and no smooth shading. |
| Who | Lua apps and wasm carts. Carts pay fuel, as for every other drawing call. |
| Objects | Built-in shapes (cube, pyramid, octahedron, sphere, torus) plus custom meshes. |
| Where | A native renderer in `acid-gfx`, integer-only, behind a small `acid_*` API. One implementation serves Lua and carts. |
| Showcase | A new app, Acid Spin, in the Llamasoft spirit. |

## Out of scope

- Textures, smooth (Gouraud) shading, more than one light, and a depth
  buffer across separate draws.
- Loading `.obj` or other model files.
- A camera API: no moving camera and no field-of-view setting.
- Clipping triangles against the near plane. A face with any point behind
  the camera is skipped instead.

## 1. Renderer (`acid-gfx`, no_std, integer-only)

### 1.1 Primitives (`raster.rs`)

- `Canvas::draw_line(x1, y1, x2, y2, color: u32)` uses Bresenham. Every
  pixel is clipped to the canvas, and coordinates are handled in i64, so
  extreme values can't overflow or panic.
- `Canvas::fill_triangle(x1, y1, x2, y2, x3, y3, color: u32)` fills by
  scanline. The vertices are sorted by y and the edges interpolated with
  integer maths. Each span is clipped to the canvas. A degenerate triangle
  (zero area) draws its outline span only, so it is never silently dropped
  and never panics. Very large coordinates are clamped before
  interpolating.

### 1.2 Fixed-point maths (`three_d.rs`)

- Angles are `u8`-range integers: 256 per full turn, taken modulo 256.
- `SIN: [i32; 256]` holds sine × 4096 (Q12), computed once into a `const`
  table: `sin(64) = 4096`, `sin(0) = 0`, `sin(128) = 0`, `sin(192) = −4096`.
  `cos(a) = SIN[(a + 64) & 255]`.
- Points are `(i32, i32, i32)` in model units.
- Rotation order is X, then Y, then Z. Each step is
  `x' = (x·cos − y·sin) >> 12` and so on, using i64 intermediates.
- Projection uses camera distance `D = 512` model units and focal
  `F = 512`. A rotated point maps to `zc = z + D`, and from there to screen
  `(cx + x·size·F/(64·zc), cy − y·size·F/(64·zc))`.
  - `size` scales only the image, not the depth, so the perspective is the
    same at every size. At `size = 64`, one model unit is one pixel at the
    screen centre (z = 0).
  - Y goes up in model space and down on screen.
  - A point is behind the camera when `zc ≤ 16`, which needs z ≤ −496.
    A built-in shape, at about ±174 after rotation, can never reach it.

### 1.3 Meshes

`Mesh { points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>, edges:
Vec<(u16, u16)> }`.

- Triangles store `u16::MAX` in their 4th slot. A quad is drawn as the two
  triangles (0, 1, 2) and (0, 2, 3).
- Edges are derived from the faces when the mesh is built: unique
  unordered pairs, in first-seen order, deduplicated in O(E log E).
- `Mesh::new(points, faces) -> Result<Mesh, MeshError>` checks, in this
  order:
  1. fewer than 3 points → Bad;
  2. over the limits (§2.3) → TooBig. This cheap check comes before any
     per-point or per-face scan, so a huge mesh is rejected early;
  3. any coordinate outside ±32767 → Bad. This keeps the fixed-point maths
     overflow-free;
  4. an index out of range, or a face that repeats an index (including a
     quad's 4th slot) → Bad.

### 1.4 Drawing a mesh

`draw_mesh(canvas, mesh, cx, cy, size, rx, ry, rz, mode, color)`:

1. Transform every point: rotate, then project. Record whether it is
   behind the camera.
2. **Wire mode (0):** draw every edge whose two ends are both in front of
   the camera, as a line in `color`.
3. **Solid mode (1):** for each triangle with all three points in front
   of the camera:
   - compute the rotated face normal (cross product, i64);
   - cull the face if it points away from the camera;
   - work out the brightness: `light = 64 + 192·max(0, n·L)/|n|` out of
     256, where `L` is a fixed unit light toward the upper-left front.
     Use an integer square root for `|n|`;
   - sort the triangles by summed rotated z, farthest first;
   - fill each one with `color` scaled per channel by `light/256`.
4. **Both mode (2):** solid, then the wire edges in `color` brightened by
   half the way to white.
5. Any other mode draws nothing.

### 1.5 Built-in shapes

`builtin(name) -> Option<Mesh>`, with the model scaled to about ±100:

| Name | Points / faces |
|---|---|
| `cube` | 8 / 6 quads |
| `pyramid` | 5 / 4 triangles + 1 quad (square base) |
| `octahedron` | 6 / 8 triangles |
| `sphere` | UV sphere, 8 segments × 6 rings: 2 poles + 5 × 8 = 42 points; 16 cap triangles + 32 quads = 48 faces |
| `torus` | 12 × 6 = 72 points / 72 quads, ring radius 70, tube radius 30 |

## 2. API

### 2.1 Lua

```
acid_draw_line(x1, y1, x2, y2, color)
acid_fill_triangle(x1, y1, x2, y2, x3, y3, color)
local id = acid_mesh_builtin(name)                 -- nil if unknown
local id, err = acid_mesh_new(points, faces)       -- nil, "bad mesh" / "too big"
acid_mesh_draw(id, x, y, size, rx, ry, rz, mode, color)
acid_mesh_free(id)
```

- `points` is a flat list `{x1, y1, z1, …}`.
- `faces` is a list of 3- or 4-element lists of 1-based indices.
- Coordinates are window-relative, like every other drawing call.
- An unknown or freed id draws nothing and freeing it does nothing.

### 2.2 Wasm carts

| Import | Signature |
|---|---|
| `draw_line` | x1, y1, x2, y2, color |
| `fill_triangle` | x1, y1, x2, y2, x3, y3, color |
| `mesh_builtin` | name_ptr, name_len → i32 (id, or −1) |
| `mesh_new` | points_ptr, n_points, faces_ptr, n_faces → i32 (id, or a negative code) |
| `mesh_draw` | id, x, y, size, rx, ry, rz, mode, color |
| `mesh_free` | id |

- `points` is an array of 3 × n_points i32s.
- `faces` is an array of 4 × n_faces i32s, 0-based, with −1 in the 4th
  slot for a triangle.
- Error codes are the existing ones: −2 bad mesh, −5 too big.
- The guest crate gets matching wrappers.

### 2.3 The store and its limits

Each running app has a mesh store, owned by its `KernelApi` and dropped
with it when the app exits. Ids are small positive integers, never reused
within a run. The limits are:

- `MESH_MAX` = 16 meshes alive;
- `MESH_POINTS_MAX` = 512 points per mesh;
- `MESH_FACES_MAX` = 1024 faces per mesh;
- `MESH_TOTAL_POINTS_MAX` = 4096 points across the app's live meshes.

A built-in shape counts toward the limits like any other mesh. The
store checks `MESH_MAX` and the total-points limit before it builds a
mesh, using a built-in's known point count, so a call over the limits
does no mesh work.

### 2.4 Fuel (carts)

- `mesh_new`: the bytes read, plus 64 fuel-bytes per face for building
  the edge list.
- `mesh_builtin`: the name's bytes, plus 64 fuel-bytes per face of the
  built-in shape. An unknown name returns −1 before any limit check.
- `draw_line`: `max(|dx|, |dy|) + 1` pixels × `BYTES_PER_PX`, capped at
  the screen.
- `fill_triangle`: the bounding box clamped to the screen, the same as a
  `fill_rect` of that box.
- `mesh_draw`:
  - a fixed base cost of 64 pixels per point and 64 per face, for
    transform, projection, culling and lighting;
  - plus, per drawn edge, the `draw_line` cost of that edge;
  - plus, per drawn triangle, its `fill_triangle` cost;
  - the total capped at 64 screens' worth of pixels.

  To keep the charge computable before drawing, it is worked out from the
  projected geometry, then charged, then the mesh is drawn.

## 3. Acid Spin (`v3/apps/acid_spin.lua`)

- An `AcidGame` app with `TICK_MS` = 33. Its manifest is `name = Acid
  Spin`, `w = 240`, `h = 200`, `resizable = true`, `min_w = 120`,
  `min_h = 100`, and it appears in the Menu.
- **Shapes:** cube, pyramid, octahedron, sphere, torus, and an "acid star"
  custom mesh. The star is a stellated octahedron built with
  `acid_mesh_new`: the octahedron's points plus 8 spike tips, as
  triangles.
- **Controls:**
  - Left/Right, or a tap in the left or right third, changes the shape;
  - Space, or a tap in the middle third, cycles wire → solid → both;
  - Up/Down change the speed, 1 to 8, starting at 3.
- **Each tick:**
  - the angles advance by `speed`, `speed·2/3` and `speed/2` for x, y
    and z;
  - the colour is `AcidPalette.hue(step)`, with `step` advancing by 2;
  - it redraws.
- **Redraw:**
  - clear the user area;
  - in wire mode, draw 4 afterimages first, from the stored previous
    angles, in the colour scaled to 25%, 40%, 55% and 70%;
  - draw the current shape;
  - draw a label row: shape name, mode and speed.

  The centre is the middle of the area below the title bar. The size
  fits the smaller of width and height.
- `on_resize` re-centres; there is no other state.
- **Test hook:** a `freeze()` method fixes the pose: cube, solid, angles
  20/30/0, colour step 0, and nothing advances. It runs when the test env
  sets `GAME_FREEZE`, or when the app is launched with the argument
  `"freeze"` (used by the golden test in the real VM), so a golden frame
  is deterministic.

## 4. Testing

### 4.1 Rust (`acid-gfx`)

- **Lines:**
  - pixel-exact for horizontal, vertical, 45°, steep, shallow, and
    reversed endpoints;
  - clipped at each edge;
  - extreme coordinates neither panic nor draw outside the canvas.
- **Triangles:**
  - pixel-exact for small flat-top, flat-bottom and general triangles;
  - shared edges between two triangles leave no gaps over a test quad;
  - degenerate triangles are handled;
  - clipping and extreme coordinates are safe.
- **Maths:** sine table values and symmetry; rotating `(100, 0, 0)` by
  `rz = 64` gives about `(0, 100, 0)`, within ±1; projecting the origin
  gives `(cx, cy)`.
- **Renderer:**
  - a wire cube draws exactly 12 edges, checked by counting distinct
    edge midpoints drawn;
  - a solid cube at a known angle shows at most 3 faces with distinct
    brightness;
  - the painter's order holds: overlapping faces show the nearer colour;
  - culling works;
  - a face behind the camera is skipped;
  - modes 0, 1 and 2 behave as specified and an invalid mode draws
    nothing;
  - each built-in shape has its point and face counts, with all edges
    derived.
- **`Mesh::new`:** every rejection case is checked.

### 4.2 Rust (API)

- **Lua:** the six calls work. Indices are 1-based. The errors are
  `"bad mesh"` and `"too big"`. Every limit is enforced. Freeing works,
  and an unknown id is a no-op.
- **Wasm:** the six imports, 0-based indices, the −1 triangle marker, the
  error codes, the import table, the count and the manual table all stay
  in sync.
- **Store:** it is dropped when the app exits, and ids are never reused.
- **Fuel:** each new charge is computed as specified and capped.

### 4.3 Lua (Acid Spin)

- Keys and taps cycle shapes and modes, and the speed is clamped between
  1 and 8.
- The colour step advances by 2 each tick.
- Afterimages are drawn only in wire mode.
- Everything drawn stays inside the window, using the existing fit
  helpers, at the opening size and after a resize.
- A resize re-centres the shape.
- The suite must fail against a stub without the behaviour.

### 4.4 Golden frame

`acid_spin.ppm` shows Acid Spin at its opening size over the desktop at
640×480, with a solid cube at fixed angles and a fixed colour
(`GAME_FREEZE`). The user approves it as a PNG first.

### 4.5 Manual check by the user

Open Acid Spin and switch through the shapes and modes. The objects
should spin smoothly, the colours cycle, the afterimages trail in wire
mode, and resizing re-centres the shape.

## 5. Docs

- `04-graphics.md`: a new section, "Lines, triangles and 3D", covering
  the primitives, meshes, the modes, the draw order for several objects,
  and the limits. It includes a runnable spinning-cube app example.
- `09-api-reference.md`: six entries, plus index entries.
- `10-wasm-carts.md`: six import rows, the array layouts, and the fuel
  notes.
- `06-games.md`: a pointer to Acid Spin as a worked example.
- `01-getting-started.md`: Acid Spin in the app list.

## Risks

- **Speed on small hardware.** A torus in solid mode is 144 triangles per
  frame. On the hosted target that is trivial. On an MCU it is bounded
  by the limits, and the renderer is integer-only.
- **Painter's algorithm artefacts.** Cyclic overlaps can be drawn in the
  wrong order. These are rare in convex built-in shapes and the torus,
  and accepted.
