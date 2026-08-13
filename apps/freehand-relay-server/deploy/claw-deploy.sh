#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <ssh-target> [ssh-key] [relay-binary]" >&2
  echo "example: $0 root@159.75.134.56 ~/Documents/server/claw.pem /path/to/linux-binary" >&2
  exit 2
}

[[ $# -ge 1 && $# -le 3 ]] || usage

ssh_target="$1"
ssh_key="${2:-}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
binary="${3:-${FREEHAND_RELAY_BINARY:-"$repo_root/target/release/freehand-relay-server"}}"
unit="$repo_root/apps/freehand-relay-server/deploy/freehand-relay.service"
env_example="$repo_root/apps/freehand-relay-server/deploy/relay.env.example"
updates_dir="$repo_root/dist/relay/updates"
evidence_dir="${FREEHAND_RELAY_DEPLOY_EVIDENCE_DIR:-"$repo_root/output/relay-deploy"}"

[[ -x "$binary" ]] || {
  echo "missing release binary: $binary" >&2
  exit 1
}
command -v file >/dev/null 2>&1 || {
  echo "missing required command: file" >&2
  exit 2
}
[[ -f "$unit" ]] || {
  echo "missing systemd unit: $unit" >&2
  exit 1
}
[[ -f "$env_example" ]] || {
  echo "missing env example: $env_example" >&2
  exit 1
}
[[ -f "$updates_dir/latest.json" ]] || {
  echo "missing relay update manifest: $updates_dir/latest.json" >&2
  exit 1
}
[[ -f "$updates_dir/freehand-android.apk" ]] || {
  echo "missing relay APK: $updates_dir/freehand-android.apk" >&2
  exit 1
}
mkdir -p "$evidence_dir"

ssh_args=(-o BatchMode=yes -o ConnectTimeout=10 -o IdentitiesOnly=yes)
scp_args=(-o BatchMode=yes -o ConnectTimeout=10 -o IdentitiesOnly=yes)
if [[ -n "$ssh_key" ]]; then
  ssh_args+=(-i "$ssh_key")
  scp_args+=(-i "$ssh_key")
fi

remote_arch="$(ssh "${ssh_args[@]}" "$ssh_target" "uname -m")"
binary_type="$(file -b "$binary")"
case "$remote_arch:$binary_type" in
  x86_64:*"ELF 64-bit LSB pie executable, x86-64"*) ;;
  aarch64:*"ELF 64-bit LSB pie executable, ARM aarch64"*) ;;
  *)
    echo "binary architecture mismatch: remote=$remote_arch binary=$binary_type" >&2
    exit 1
    ;;
esac

remote_tmp="/tmp/freehand-relay-deploy-$$"
ssh "${ssh_args[@]}" "$ssh_target" "mkdir -p '$remote_tmp'"
scp "${scp_args[@]}" "$binary" "$ssh_target:$remote_tmp/freehand-relay-server"
scp "${scp_args[@]}" "$unit" "$ssh_target:$remote_tmp/freehand-relay.service"
scp "${scp_args[@]}" "$env_example" "$ssh_target:$remote_tmp/relay.env.example"
scp "${scp_args[@]}" "$updates_dir/latest.json" "$ssh_target:$remote_tmp/latest.json"
scp "${scp_args[@]}" "$updates_dir/freehand-android.apk" "$ssh_target:$remote_tmp/freehand-android.apk"

ssh "${ssh_args[@]}" "$ssh_target" "
  set -euo pipefail
  id freehand-relay >/dev/null 2>&1 || useradd --system --home /var/lib/freehand-relay --shell /usr/sbin/nologin freehand-relay
  install -d -o freehand-relay -g freehand-relay /var/lib/freehand-relay /var/lib/freehand-relay/updates /var/lib/freehand-relay/account-config /etc/freehand-relay
  install -m 0755 '$remote_tmp/freehand-relay-server' /tmp/freehand-relay-server.next
  mv -f /tmp/freehand-relay-server.next /usr/local/bin/freehand-relay-server
  install -m 0644 '$remote_tmp/freehand-relay.service' /etc/systemd/system/freehand-relay.service
  install -m 0600 '$remote_tmp/relay.env.example' /etc/freehand-relay/relay.env
  install -m 0644 '$remote_tmp/latest.json' /var/lib/freehand-relay/updates/latest.json
  install -m 0644 '$remote_tmp/freehand-android.apk' /var/lib/freehand-relay/updates/freehand-android.apk
  chown -R freehand-relay:freehand-relay /var/lib/freehand-relay
  if [[ ! -f /var/lib/freehand-relay/store.json ]]; then
    set -a
    . /etc/freehand-relay/relay.env
    set +a
    /usr/local/bin/freehand-relay-server init-store
    chown freehand-relay:freehand-relay /var/lib/freehand-relay/store.json
  fi
  systemctl daemon-reload
  systemctl enable freehand-relay.service
  systemctl restart freehand-relay.service
  systemctl is-active --quiet freehand-relay.service
"

local_binary_sha256="$(shasum -a 256 "$binary" | cut -d ' ' -f 1)"
local_manifest_sha256="$(shasum -a 256 "$updates_dir/latest.json" | cut -d ' ' -f 1)"
local_apk_sha256="$(shasum -a 256 "$updates_dir/freehand-android.apk" | cut -d ' ' -f 1)"
remote_evidence="$(
  ssh "${ssh_args[@]}" "$ssh_target" "
    set -euo pipefail
    remote_binary_sha256=\$(sha256sum /usr/local/bin/freehand-relay-server | cut -d ' ' -f 1)
    remote_unit_sha256=\$(sha256sum /etc/systemd/system/freehand-relay.service | cut -d ' ' -f 1)
    remote_env_sha256=\$(sha256sum /etc/freehand-relay/relay.env | cut -d ' ' -f 1)
    remote_manifest_sha256=\$(sha256sum /var/lib/freehand-relay/updates/latest.json | cut -d ' ' -f 1)
    remote_apk_sha256=\$(sha256sum /var/lib/freehand-relay/updates/freehand-android.apk | cut -d ' ' -f 1)
    remote_manifest_size=\$(wc -c < /var/lib/freehand-relay/updates/latest.json | tr -d ' ')
    remote_apk_size=\$(wc -c < /var/lib/freehand-relay/updates/freehand-android.apk | tr -d ' ')
    served_apk_path=/tmp/freehand-relay-served.apk
    service_pid=\$(systemctl show -p MainPID --value freehand-relay.service)
    service_exec_start=\$(systemctl show -p ExecStart --value freehand-relay.service)
    service_environment_files=\$(systemctl show -p EnvironmentFiles --value freehand-relay.service)
    health_body=\$(curl --fail --silent http://127.0.0.1:19091/relay/health)
    manifest_body=\$(curl --fail --silent http://127.0.0.1:19091/relay/updates/latest.json)
    curl --fail --silent http://127.0.0.1:19091/relay/updates/freehand-android.apk -o \"\$served_apk_path\"
    served_apk_sha256=\$(sha256sum \"\$served_apk_path\" | cut -d ' ' -f 1)
    served_apk_size=\$(wc -c < \"\$served_apk_path\" | tr -d ' ')
    printf 'remote_binary_sha256=%s\\n' \"\$remote_binary_sha256\"
    printf 'remote_unit_sha256=%s\\n' \"\$remote_unit_sha256\"
    printf 'remote_env_sha256=%s\\n' \"\$remote_env_sha256\"
    printf 'remote_manifest_sha256=%s\\n' \"\$remote_manifest_sha256\"
    printf 'remote_apk_sha256=%s\\n' \"\$remote_apk_sha256\"
    printf 'remote_manifest_size=%s\\n' \"\$remote_manifest_size\"
    printf 'remote_apk_size=%s\\n' \"\$remote_apk_size\"
    printf 'served_apk_sha256=%s\\n' \"\$served_apk_sha256\"
    printf 'served_apk_size=%s\\n' \"\$served_apk_size\"
    printf 'service_pid=%s\\n' \"\$service_pid\"
    printf 'service_exec_start=%s\\n' \"\$service_exec_start\"
    printf 'service_environment_files=%s\\n' \"\$service_environment_files\"
    printf 'env_keys_present='
    awk -F= '/^FREEHAND_RELAY_(BIND|STORE|PRESENCE_LEASE_SECONDS|SECURE_COOKIE|UPDATES_DIR|ACCOUNT_CONFIG_DIR)=/ { printf \"%s,\", \$1 }' /etc/freehand-relay/relay.env
    printf '\\n'
    printf 'health_body=%s\\n' \"\$health_body\"
    printf 'manifest_body=%s\\n' \"\$manifest_body\"
    test -d /var/lib/freehand-relay/account-config
    test -f /var/lib/freehand-relay/store.json
    printf 'account_config_dir_present=true\\n'
    printf 'store_present=true\\n'
    rm -f \"\$served_apk_path\"
  "
)"
printf '%s\n' "$remote_evidence" >"$evidence_dir/remote.txt"

remote_binary_sha256="$(sed -n 's/^remote_binary_sha256=//p' "$evidence_dir/remote.txt")"
remote_manifest_sha256="$(sed -n 's/^remote_manifest_sha256=//p' "$evidence_dir/remote.txt")"
remote_apk_sha256="$(sed -n 's/^remote_apk_sha256=//p' "$evidence_dir/remote.txt")"
served_apk_sha256="$(sed -n 's/^served_apk_sha256=//p' "$evidence_dir/remote.txt")"
served_apk_size="$(sed -n 's/^served_apk_size=//p' "$evidence_dir/remote.txt")"
[[ "$remote_binary_sha256" == "$local_binary_sha256" ]] || {
  echo "remote binary SHA-256 mismatch local=$local_binary_sha256 remote=$remote_binary_sha256" >&2
  exit 1
}
[[ "$remote_manifest_sha256" == "$local_manifest_sha256" ]] || {
  echo "remote manifest SHA-256 mismatch local=$local_manifest_sha256 remote=$remote_manifest_sha256" >&2
  exit 1
}
[[ "$remote_apk_sha256" == "$local_apk_sha256" ]] || {
  echo "remote APK SHA-256 mismatch local=$local_apk_sha256 remote=$remote_apk_sha256" >&2
  exit 1
}
[[ "$served_apk_sha256" == "$local_apk_sha256" ]] || {
  echo "served APK SHA-256 mismatch local=$local_apk_sha256 served=$served_apk_sha256" >&2
  exit 1
}
local_apk_size="$(wc -c < "$updates_dir/freehand-android.apk" | tr -d ' ')"
[[ "$served_apk_size" == "$local_apk_size" ]] || {
  echo "served APK size mismatch local=$local_apk_size served=$served_apk_size" >&2
  exit 1
}
jq -e '
  (.versionCode | type == "number" and . > 0) and
  (.sha256 | type == "string" and test("^[0-9a-f]{64}$")) and
  (.size | type == "number" and . > 0) and
  (.signerSha256 | type == "string" and test("^[0-9a-f]{64}$"))
' "$updates_dir/latest.json" >/dev/null

remote_manifest_body="$(sed -n 's/^manifest_body=//p' "$evidence_dir/remote.txt")"
remote_health_body="$(sed -n 's/^health_body=//p' "$evidence_dir/remote.txt")"
[[ "$remote_health_body" == "ok" ]] || {
  echo "remote Relay health failed: $remote_health_body" >&2
  exit 1
}
jq -e '
  (.versionCode == input.versionCode) and
  (.sha256 == input.sha256) and
  (.size == input.size) and
  (.signerSha256 == input.signerSha256)
' "$updates_dir/latest.json" <(printf '%s\n' "$remote_manifest_body") >/dev/null

printf 'claw_relay_deploy_ok target=%s binary_sha256=%s manifest_sha256=%s apk_sha256=%s evidence=%s\n' \
  "$ssh_target" "$local_binary_sha256" "$local_manifest_sha256" "$local_apk_sha256" \
  "$evidence_dir/remote.txt"
