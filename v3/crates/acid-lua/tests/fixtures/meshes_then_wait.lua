acid_mesh_builtin("cube")
acid_mesh_new({ 0, 0, 0,  100, 0, 0,  0, 100, 0 }, { { 1, 2, 3 } })
while acid_poll_event(50) ~= "close" do end
