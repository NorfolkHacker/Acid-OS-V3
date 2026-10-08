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
eval "$(xdotool getwindowgeometry --shell "$wid")"
xdotool windowactivate --sync "$wid" 2>/dev/null || true

full="$(mktemp -u --suffix=.png)"
trap 'rm -f "$full"' EXIT
spectacle -b -n -f -o "$full" -d "${DELAY_MS:-600}" >/dev/null 2>&1
[ -s "$full" ] || { echo "spectacle produced nothing -- is the KDE session alive?" >&2; exit 1; }

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
