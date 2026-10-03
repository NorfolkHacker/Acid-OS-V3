-- Golden-test probe: a bare window. The overlay it stands for (two
-- rectangles, one partly off-screen) is drawn by the test through the
-- kernel, because a script outside v3/apps/ is cart-level and a cart may
-- not open the overlay (spec §16.2). The title is part of the
-- committed golden frame, so keep it.
local OverlayProbeApp = AcidApp:extend("OverlayProbeApp")

OverlayProbeApp:new():start()
