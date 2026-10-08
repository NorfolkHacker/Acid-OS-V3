#!/usr/bin/env bash
# Set up (or refresh) this machine's private copy of the C library headers.
#
# SteamOS ships its OS image with /usr/include stripped, so any crate that
# compiles C cannot build. Acid OS hits this through mlua's "vendored" feature,
# which compiles the Lua 5.4 C sources: gcc's own limits.h gets as far as its
# `#include_next <limits.h>` and finds no glibc header to hand off to.
#
# This fetches the headers belonging to the glibc already installed and unpacks
# them under $HOME, then writes the .cargo/config.toml that points cc at them.
# The headers go in $HOME rather than /usr because a SteamOS update replaces the
# whole OS image: anything restored into /usr is wiped by the next update, and a
# home folder is left alone. Needs no root.
#
# Run it once after cloning, and again after an update that bumps glibc so the
# headers match the glibc actually on the system.
#
#   SYSROOT=...            where to put the headers (default ~/.local/sysroot)
#   WRITE_CARGO_CONFIG=0   install the headers but leave .cargo/config.toml alone
set -euo pipefail

SYSROOT="${SYSROOT:-$HOME/.local/sysroot}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG="$REPO/.cargo/config.toml"
MARKER="# written by scripts/refresh-sysroot.sh"

for tool in curl tar zstd pacman cc; do
  command -v "$tool" >/dev/null || { echo "need $tool" >&2; exit 1; }
done

echo "resolving packages for the installed glibc / linux-api-headers..."
mapfile -t urls < <(pacman -Sp glibc linux-api-headers)
[ "${#urls[@]}" -eq 2 ] || { echo "expected 2 package urls, got ${#urls[@]}" >&2; exit 1; }
printf '  %s\n' "${urls[@]}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for url in "${urls[@]}"; do
  echo "downloading $(basename "$url")"
  curl -fsSL -o "$tmp/$(basename "$url")" "$url"
done

echo "extracting headers..."
mkdir -p "$tmp/stage"
for pkg in "$tmp"/*.pkg.tar.zst; do
  tar -I zstd -xf "$pkg" -C "$tmp/stage" usr/include
done

count=$(find "$tmp/stage/usr/include" -type f | wc -l)
[ "$count" -gt 1000 ] || { echo "only $count headers extracted, refusing" >&2; exit 1; }

# Staged fully before touching $SYSROOT, so a failed download or a short
# extract can never leave a half-populated sysroot behind.
echo "installing $count headers to $SYSROOT"
rm -rf "$SYSROOT/usr/include"
mkdir -p "$SYSROOT/usr"
cp -a "$tmp/stage/usr/include" "$SYSROOT/usr/"

echo "verifying a compile against them..."
printf '#include <limits.h>\n#include <stdio.h>\nint main(void){printf("%%d\\n",INT_MAX);return 0;}\n' > "$tmp/probe.c"
cc -isystem "$SYSROOT/usr/include" "$tmp/probe.c" -o "$tmp/probe"
"$tmp/probe" >/dev/null

if [ "${WRITE_CARGO_CONFIG:-1}" = 0 ]; then
  echo
  echo "ok: $SYSROOT/usr/include ($count headers)"
  echo "not writing $CONFIG (WRITE_CARGO_CONFIG=0). Cargo needs:"
  echo "  CFLAGS=\"-isystem $SYSROOT/usr/include\""
  exit 0
fi

# The config holds an absolute path, so it is per-machine and git-ignored --
# hence generated here rather than committed. Only ever rewrite our own file:
# anything else in .cargo/config.toml is the user's and is left untouched.
if [ -e "$CONFIG" ] && ! grep -qF "$MARKER" "$CONFIG"; then
  echo
  echo "ok: $SYSROOT/usr/include ($count headers)"
  echo "$CONFIG already exists and was not written by this script, so it has"
  echo "been left alone. Add this to it by hand:"
  echo
  echo "  [env]"
  echo "  CFLAGS = \"-isystem $SYSROOT/usr/include\""
  echo "  CXXFLAGS = \"-isystem $SYSROOT/usr/include\""
  exit 0
fi

# At the repository root on purpose: cargo looks for .cargo/config.toml by
# walking up from the current directory and ignores --manifest-path when it
# does, so a copy under v3/ would be skipped by the documented command
# (`cargo run --manifest-path v3/Cargo.toml -p acid-os`) run from the top.
mkdir -p "$(dirname "$CONFIG")"
cat > "$CONFIG" <<EOF
$MARKER -- re-run that script after a SteamOS update that bumps glibc.
#
# SteamOS ships its OS image without the C library headers, so the bundled
# Lua cannot build. They live in \$HOME instead, and this points cc at them.
#
# Machine-specific (absolute path below), so this file is git-ignored.
[env]
CFLAGS = "-isystem $SYSROOT/usr/include"
CXXFLAGS = "-isystem $SYSROOT/usr/include"
EOF

echo
echo "ok: $SYSROOT/usr/include ($count headers), test program compiled and ran"
echo "wrote $CONFIG"
echo "Acid OS will now build: cargo run --release --manifest-path v3/Cargo.toml -p acid-os"
