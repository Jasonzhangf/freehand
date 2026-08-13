#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <tailscale-manifest-url> <relay-manifest-url>" >&2
  exit 2
}

[[ $# -eq 2 ]] || usage

verify_dual_path_update() {
  :
}
verify_dual_path_update

tailscale_manifest_url="$1"
relay_manifest_url="$2"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

curl -fsS "$tailscale_manifest_url" -o "$tmp_dir/tailscale.json"
curl -fsS "$relay_manifest_url" -o "$tmp_dir/relay.json"

for manifest in tailscale relay; do
  jq -e '
    (.versionCode | type == "number" and . > 0) and
    (.versionName | type == "string" and length > 0) and
    (.apkUrl | type == "string" and length > 0) and
    (.sha256 | type == "string" and test("^[0-9a-f]{64}$")) and
    (.size | type == "number" and . > 0) and
    (.signerSha256 | type == "string" and test("^[0-9a-f]{64}$"))
  ' "$tmp_dir/$manifest.json" >/dev/null

done

tailscale_version="$(jq -r '.versionCode' "$tmp_dir/tailscale.json")"
relay_version="$(jq -r '.versionCode' "$tmp_dir/relay.json")"
tailscale_sha="$(jq -r '.sha256' "$tmp_dir/tailscale.json")"
relay_sha="$(jq -r '.sha256' "$tmp_dir/relay.json")"
tailscale_size="$(jq -r '.size' "$tmp_dir/tailscale.json")"
relay_size="$(jq -r '.size' "$tmp_dir/relay.json")"
tailscale_signer_sha256="$(jq -r '.signerSha256' "$tmp_dir/tailscale.json")"
relay_signer_sha256="$(jq -r '.signerSha256' "$tmp_dir/relay.json")"

[[ "$tailscale_version" == "$relay_version" ]] || {
  echo "versionCode mismatch tailscale=$tailscale_version relay=$relay_version" >&2
  exit 1
}
[[ "$tailscale_sha" == "$relay_sha" ]] || {
  echo "sha256 mismatch tailscale=$tailscale_sha relay=$relay_sha" >&2
  exit 1
}
[[ "$tailscale_size" == "$relay_size" ]] || {
  echo "size mismatch tailscale=$tailscale_size relay=$relay_size" >&2
  exit 1
}
[[ "$tailscale_signer_sha256" == "$relay_signer_sha256" ]] || {
  echo "signerSha256 mismatch tailscale=$tailscale_signer_sha256 relay=$relay_signer_sha256" >&2
  exit 1
}

resolve_apk_url() {
  local manifest_url="$1"
  local apk_url="$2"
  node -e 'console.log(new URL(process.argv[2], process.argv[1]).toString())' "$manifest_url" "$apk_url"
}

find_apksigner() {
  if command -v apksigner >/dev/null 2>&1; then
    command -v apksigner
    return 0
  fi
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    find "$ANDROID_HOME/build-tools" -name apksigner -type f 2>/dev/null | sort | tail -n 1
    return 0
  fi
  return 1
}

apk_signer_sha256() {
  local apk_path="$1"
  local apksigner_bin
  apksigner_bin="$(find_apksigner)"
  [[ -n "$apksigner_bin" ]] || {
    echo "missing required command: apksigner; set ANDROID_HOME or install Android build-tools" >&2
    exit 2
  }
  "$apksigner_bin" verify --verbose --print-certs "$apk_path" \
    | sed -n 's/^Signer #1 certificate SHA-256 digest: //p' \
    | tr 'A-Z' 'a-z' \
    | head -n 1
}

tailscale_apk_url="$(resolve_apk_url "$tailscale_manifest_url" "$(jq -r '.apkUrl' "$tmp_dir/tailscale.json")")"
relay_apk_url="$(resolve_apk_url "$relay_manifest_url" "$(jq -r '.apkUrl' "$tmp_dir/relay.json")")"

curl -fsS "$tailscale_apk_url" -o "$tmp_dir/tailscale.apk"
curl -fsS "$relay_apk_url" -o "$tmp_dir/relay.apk"

actual_tailscale_sha="$(shasum -a 256 "$tmp_dir/tailscale.apk" | cut -d ' ' -f 1)"
actual_relay_sha="$(shasum -a 256 "$tmp_dir/relay.apk" | cut -d ' ' -f 1)"
actual_tailscale_size="$(wc -c < "$tmp_dir/tailscale.apk" | tr -d ' ')"
actual_relay_size="$(wc -c < "$tmp_dir/relay.apk" | tr -d ' ')"
actual_tailscale_signer_sha256="$(apk_signer_sha256 "$tmp_dir/tailscale.apk")"
actual_relay_signer_sha256="$(apk_signer_sha256 "$tmp_dir/relay.apk")"

[[ "$actual_tailscale_sha" == "$tailscale_sha" ]] || {
  echo "tailscale APK hash mismatch manifest=$tailscale_sha actual=$actual_tailscale_sha" >&2
  exit 1
}
[[ "$actual_relay_sha" == "$relay_sha" ]] || {
  echo "relay APK hash mismatch manifest=$relay_sha actual=$actual_relay_sha" >&2
  exit 1
}
[[ "$actual_tailscale_size" == "$tailscale_size" ]] || {
  echo "tailscale APK size mismatch manifest=$tailscale_size actual=$actual_tailscale_size" >&2
  exit 1
}
[[ "$actual_relay_size" == "$relay_size" ]] || {
  echo "relay APK size mismatch manifest=$relay_size actual=$actual_relay_size" >&2
  exit 1
}
[[ "$actual_tailscale_signer_sha256" == "$tailscale_signer_sha256" ]] || {
  echo "tailscale APK signer mismatch manifest=$tailscale_signer_sha256 actual=$actual_tailscale_signer_sha256" >&2
  exit 1
}
[[ "$actual_relay_signer_sha256" == "$relay_signer_sha256" ]] || {
  echo "relay APK signer mismatch manifest=$relay_signer_sha256 actual=$actual_relay_signer_sha256" >&2
  exit 1
}
[[ "$actual_tailscale_signer_sha256" == "$actual_relay_signer_sha256" ]] || {
  echo "downloaded APK signer mismatch tailscale=$actual_tailscale_signer_sha256 relay=$actual_relay_signer_sha256" >&2
  exit 1
}

printf 'dual_path_update_ok versionCode=%s sha256=%s size=%s signerSha256=%s tailscale=%s relay=%s\n' \
  "$tailscale_version" "$tailscale_sha" "$tailscale_size" "$tailscale_signer_sha256" \
  "$tailscale_manifest_url" "$relay_manifest_url"
