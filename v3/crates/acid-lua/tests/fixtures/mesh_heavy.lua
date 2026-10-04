-- A loop of heavy mesh draws that never polls: 1024 copies of a quad that
-- fills the window, drawn solid. Each draw is long, but few instructions.
local pts = { -20, -20, -50,  -20, 20, -50,  20, 20, -50,  20, -20, -50 }
local faces = {}
for i = 1, 1024 do faces[i] = { 1, 2, 3, 4 } end
local id = acid_mesh_new(pts, faces)
while true do acid_mesh_draw(id, 80, 60, 6400, 0, 0, 0, 1, 0xFFFFFF) end
