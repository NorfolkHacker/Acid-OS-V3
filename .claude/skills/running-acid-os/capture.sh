#!/usr/bin/env bash
# Capture the running Acid OS window to a PNG.
#
# Why not ffmpeg x11grab: this is a KDE Wayland session, and KWin composites
# XWayland surfaces itself. Grabbing the X11 root returns a fully black frame
# even though the app is drawing correctly. spectacle captures the real screen
# through the compositor; we then crop to the window.
#
#   ./capture.sh [out.png]        DELAY_MS=600 by default
set -euo pipefail

out="${1:-/tmp/acid-window.png}"

wid=$(xdotool search --name "Acid OS" 2>/dev/null | head -1 || true)
if [ -z "$wid" ]; then
  echo "no 'Acid OS' window on X11." >&2
  echo "Launch it with 'env -u WAYLAND_DISPLAY' or it goes to native Wayland," >&2
  echo "where xdotool and this script cannot see it. See SKILL.md." >&2
  exit 1
fi

# Sets WINDOW, X, Y, WIDTH, HEIGHT, SCREEN. X/Y are desktop coordinates, which
# is what we crop by -- spectacle's fullscreen grab spans all monitors.
# Raise, but deliberately do NOT activate. The grab below reads whatever is
# painted at these screen coordinates, so an obscured window captures the window
# on top of it instead -- a convincing 800x600 PNG that is not the app at all.
# windowactivate would also fix that, but it asks KWin to re-stack AND can move
# the window between this geometry read and the grab, misaligning the crop and
# invalidating any coordinates the caller computed for a click. windowraise
# brings it to the front without moving it.
xdotool windowraise "$wid" 2>/dev/null || true
eval "$(xdotool getwindowgeometry --shell "$wid")"

full="$(mktemp -u --suffix=.png)"
trap 'rm -f "$full"' EXIT
spectacle -b -n -f -o "$full" -d "${DELAY_MS:-600}" >/dev/null 2>&1
[ -s "$full" ] || { echo "spectacle produced nothing -- is the KDE session alive?" >&2; exit 1; }

# Bounds-check before cropping. ffmpeg CLAMPS an out-of-range crop instead of
# erroring: ask for 800x600 at x=1013 of a 1280-wide grab and it silently hands
# back 800x600 taken from x=480, i.e. a different part of the desktop entirely.
# That looks like a screenshot of the app and is not one -- it is how an
# off-screen window gets mistaken for the window having moved.
IFS=, read -r FW FH < <(ffprobe -hide_banner -loglevel error \
  -show_entries stream=width,height -of csv=p=0 "$full")
if [ $((X + WIDTH)) -gt "$FW" ] || [ $((Y + HEIGHT)) -gt "$FH" ] || [ "$X" -lt 0 ] || [ "$Y" -lt 0 ]; then
  echo "window is not fully on screen: ${WIDTH}x${HEIGHT} at +${X},+${Y}, screen is ${FW}x${FH}." >&2
  echo "Cropping that would silently capture the wrong region. Move the window" >&2
  echo "fully on screen (xdotool windowmove) or relaunch with a --screen size" >&2
  echo "that fits, then capture again." >&2
  exit 1
fi

ffmpeg -hide_banner -loglevel error -i "$full" -vf "crop=$WIDTH:$HEIGHT:$X:$Y" -y "$out"

# A near-monochrome result means the capture path failed, not that the app drew
# nothing. Fail loudly instead of handing back a black PNG that reads as "ran".
colours=$(ffmpeg -hide_banner -loglevel error -i "$out" -vf format=rgb24 -f rawvideo - 2>/dev/null \
  | od -An -tx1 -v \
  | awk '{for (i = 1; i + 2 <= NF; i += 3) c[$i" "$(i+1)" "$(i+2)]++} END {print length(c)}')

echo "$out  ${WIDTH}x${HEIGHT} at +${X},${Y}  ${colours} distinct colours"
if [ "${colours:-0}" -lt 8 ]; then
  echo "that capture is blank -- the compositor grab failed. See SKILL.md." >&2
  exit 1
fi
