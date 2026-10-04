-- The built-in apps size themselves from acid_screen_size(), not 640x360.
eq(DesktopApp.SCREEN_W, 800, "the desktop spans the screen")
eq(DesktopApp.MAX_TASKBAR_SLOTS, 10, "(800 - 60 - 90) // 60 window buttons fit")
eq(Cartfile.MAX_W, 800, "a cart window may be as wide as the screen")
eq(Cartfile.MAX_H, 576, "and as tall as the screen less the desktop strip")
eq({ AcidEggs.SCREEN_W, AcidEggs.SCREEN_H }, { 800, 600 }, "the eggs fly across the whole screen")
