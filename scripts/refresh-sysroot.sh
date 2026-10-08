#!/usr/bin/env bash
# Rebuild this machine's private copy of the C library headers.
#
# SteamOS ships its OS image with /usr/include stripped, so any crate that
# compiles C (here: mlua's vendored Lua 5.4) cannot build. We keep the headers
# under $HOME instead of restoring them into /usr, because a SteamOS update
# replaces the whole OS image and would wipe them again.
#
# Run this after a SteamOS update that bumps glibc, so the headers match the
# glibc actually on the system. Needs no root.
set -euo pipefail

SYSROOT="${SYSROOT:-$HOME/.local/sysroot}"

for tool in curl tar zstd pacman; do
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

echo "installing $count headers to $SYSROOT"
rm -rf "$SYSROOT/usr/include"
mkdir -p "$SYSROOT/usr"
cp -a "$tmp/stage/usr/include" "$SYSROOT/usr/"

echo "verifying a compile against them..."
printf '#include <limits.h>\n#include <stdio.h>\nint main(void){printf("%%d\\n",INT_MAX);return 0;}\n' > "$tmp/probe.c"
cc -isystem "$SYSROOT/usr/include" "$tmp/probe.c" -o "$tmp/probe"
"$tmp/probe" >/dev/null

echo "ok: $SYSROOT/usr/include ($count headers), test program compiled and ran"
echo "cargo picks this up via the repo-root .cargo/config.toml -> CFLAGS"
