-- A reader function runs inside load's own protected parser, which turns
-- the watchdog error into nil, msg. Looping over load must still end.
while true do
  load(function() while true do end end)
end
