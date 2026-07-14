#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
source_logo="$repo_root/assets/logo.png"
res_root="$repo_root/apps/freehand-android/app/src/main/res"

command -v magick >/dev/null 2>&1 || {
  echo "verify-launcher-icons: ImageMagick 'magick' is required" >&2
  exit 1
}

test -f "$source_logo" || {
  echo "verify-launcher-icons: missing source logo: $source_logo" >&2
  exit 1
}

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

verify_launcher_icons() {
  local densities=(mdpi hdpi xhdpi xxhdpi xxxhdpi)
  local sizes=(48 72 96 144 192)
  local index density size expected name actual dimensions pixels

  for index in "${!densities[@]}"; do
    density="${densities[$index]}"
    size="${sizes[$index]}"
    expected="$tmp_dir/expected-${density}.png"
    magick "$source_logo" -resize "${size}x${size}!" -strip "$expected"
    for name in ic_launcher.png ic_launcher_round.png; do
      actual="$res_root/mipmap-$density/$name"
      test -f "$actual" || {
        echo "verify-launcher-icons: missing $actual" >&2
        exit 1
      }
      dimensions="$(magick identify -format '%wx%h' "$actual")"
      test "$dimensions" = "${size}x${size}" || {
        echo "verify-launcher-icons: expected ${size}x${size}, got $dimensions: $actual" >&2
        exit 1
      }
      if ! magick compare -metric AE "$expected" "$actual" null: 2>"$tmp_dir/compare.txt"; then
        pixels="$(tr -d '[:space:]' <"$tmp_dir/compare.txt")"
        echo "verify-launcher-icons: source pixel drift ($pixels pixels): $actual" >&2
        exit 1
      fi
    done
  done
}

verify_launcher_icons
echo "verify-launcher-icons: all Android launcher icons match assets/logo.png"
