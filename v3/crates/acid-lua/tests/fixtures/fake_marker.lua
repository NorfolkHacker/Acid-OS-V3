pcall(function() error("acid: stopped responding") end)
acid_fill_rect(0, 0, 10, 10, 0xFF0000)
while acid_poll_event(50) ~= "close" do end
