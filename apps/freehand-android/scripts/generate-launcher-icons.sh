#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
source_logo="$repo_root/assets/logo.png"
res_root="$repo_root/apps/freehand-android/app/src/main/res"

command -v magick >/dev/null 2>&1 || {
  echo "generate-launcher-icons: ImageMagick 'magick' is required" >&2
  exit 1
}

test -f "$source_logo" || {
  echo "generate-launcher-icons: missing source logo: $source_logo" >&2
  exit 1
}

generate_launcher_icons() {
  local densities=(mdpi hdpi xhdpi xxhdpi xxxhdpi)
  local sizes=(48 72 96 144 192)
  local index density size output_dir name

  for index in "${!densities[@]}"; do
    density="${densities[$index]}"
    size="${sizes[$index]}"
    output_dir="$res_root/mipmap-$density"
    mkdir -p "$output_dir"
    for name in ic_launcher.png ic_launcher_round.png; do
      magick "$source_logo" -resize "${size}x${size}!" -strip "$output_dir/$name"
    done
  done
}

generate_launcher_icons
echo "generate-launcher-icons: derived Android launcher icons from assets/logo.png"
