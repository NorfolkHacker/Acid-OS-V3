-- One load whose reader spins, then a well-behaved poll loop: the stop
-- inside the reader must still end the app.
load(function() while true do end end)
while acid_poll_event(50) ~= "close" do end
