Carts -- programs written outside Acid OS and carried in.

A .cart is a plain Lua app (see hello_acid.cart) that the Load Cart app
installs into v3/apps as <slug>.lua plus a generated <slug>.app.toml. This
directory is one of the roots Load Cart browses; the others are ~/carts
and the host's /media, /mnt and /run/media mount points, so a cart on a
USB stick or SD card shows up in the same list.

Header (all lines optional, first comment block only):

  -- name: Hello Acid            window title and Menu entry
  -- w: 200                      window width,  clamped to the screen
  -- h: 150                      window height, clamped to the screen
  -- desc: one-line description  shown in the Menu and File Manager
  -- libs: lib/acid_palette.lua  modules from v3/apps/lib, comma separated
  -- category: game              listed under Games in the File Manager;
                                 anything else (or none) is listed under Apps
  -- menu: false                 left out of the Menu dropdown; still opens
                                 from File Manager and Terminal's `run`

An installed cart joins the Menu dropdown at the next boot; Load Cart's
RUN button starts it immediately in the meantime.

WASM carts: a .wasm module (see hello_wasm.wasm) installs the same way,
as <slug>.wasm plus a manifest with `runtime = wasm`. It always runs at
cart level. Its header is a custom section named `acid` holding the same
`key: value` lines as above, without the `-- ` prefix (libs is ignored);
a module without one installs under its filename at the default size.
Write WASM carts in Rust with the acid-cart crate in v3/carts-src, which
provides the imports, the exports (acid_cart! macro) and a Cart trait;
v3/carts-src/hello-wasm is the source of hello_wasm.wasm.
