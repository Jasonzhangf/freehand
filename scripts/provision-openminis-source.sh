#!/usr/bin/env bash
set -euo pipefail

unset $(git rev-parse --local-env-vars)

run_provision_openminis_source() {
repo_root="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
manifest="$repo_root/docs/migrations/openminis-ui/ui-tree.manifest.json"
destination="$repo_root/external/OpenMinis"
repository_url="https://github.com/OpenMinis/OpenMinis.git"

command -v git >/dev/null
command -v jq >/dev/null

commit="$(
  jq -er '
    .source_repository.commit
    | select(type == "string" and test("^[0-9a-f]{40}$"))
  ' "$manifest"
)"
source_paths=()
while IFS= read -r source_path; do
  source_paths+=("$source_path")
done < <(jq -er '.nodes[].source_paths[]' "$manifest" | sort -u)
[[ "${#source_paths[@]}" -gt 0 ]] || {
  printf 'OpenMinis manifest has no source paths: %s\n' "$manifest" >&2
  exit 1
}

verify_checkout() {
  local checkout="$1"
  local inside_worktree
  inside_worktree="$(git -C "$checkout" rev-parse --is-inside-work-tree 2>/dev/null || true)"
  [[ "$inside_worktree" == true ]] || {
    printf 'OpenMinis source path exists but is not a Git checkout: %s\n' "$checkout" >&2
    return 1
  }
  local checkout_root
  checkout_root="$(git -C "$checkout" rev-parse --show-toplevel)"
  [[ "$(cd "$checkout_root" && pwd -P)" == "$(cd "$checkout" && pwd -P)" ]] || {
    printf 'OpenMinis source path is not the checkout root: %s\n' "$checkout" >&2
    return 1
  }
  local origin
  origin="$(git -C "$checkout" remote get-url origin)"
  case "$origin" in
    https://github.com/OpenMinis/OpenMinis|https://github.com/OpenMinis/OpenMinis.git) ;;
    *)
      printf 'OpenMinis origin drift: expected %s, got %s\n' "$repository_url" "$origin" >&2
      return 1
      ;;
  esac
  local head
  head="$(git -C "$checkout" rev-parse HEAD)"
  [[ "$head" == "$commit" ]] || {
    printf 'OpenMinis HEAD drift: expected %s, got %s\n' "$commit" "$head" >&2
    return 1
  }
  [[ -z "$(git -C "$checkout" status --porcelain)" ]] || {
    printf 'OpenMinis checkout has uncommitted or untracked changes: %s\n' "$checkout" >&2
    return 1
  }
}

if [[ -e "$destination" ]]; then
  verify_checkout "$destination"
  printf 'OpenMinis source ready at %s (%s)\n' "$destination" "$commit"
  exit 0
fi

mkdir -p "$repo_root/external"
lock="$repo_root/external/.OpenMinis.provision.lock"
lock_acquired=false
lock_owner="$(hostname):$$"
staging=""
cleanup() {
  if [[ -n "$staging" && -e "$staging" ]]; then
    rm -rf -- "$staging"
  fi
  if [[ "$lock_acquired" == true && -L "$lock" && "$(readlink "$lock")" == "$lock_owner" ]]; then
    rm -- "$lock"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP

for _ in $(seq 1 600); do
  if ln -s "$lock_owner" "$lock" 2>/dev/null; then
    lock_acquired=true
    break
  fi
  if [[ -e "$destination" ]]; then
    verify_checkout "$destination"
    printf 'OpenMinis source ready at %s (%s)\n' "$destination" "$commit"
    exit 0
  fi
  if [[ -L "$lock" ]]; then
    observed_owner="$(readlink "$lock")"
    observed_host="${observed_owner%:*}"
    observed_pid="${observed_owner##*:}"
    if [[ "$observed_host" == "$(hostname)" && "$observed_pid" =~ ^[0-9]+$ ]] \
      && ! kill -0 "$observed_pid" 2>/dev/null \
      && [[ -L "$lock" && "$(readlink "$lock")" == "$observed_owner" ]]; then
      rm -- "$lock"
      continue
    fi
  fi
  sleep 0.1
done
[[ "$lock_acquired" == true ]] || {
  printf 'Timed out waiting for OpenMinis provisioning lock: %s\n' "$lock" >&2
  exit 1
}

if [[ -e "$destination" ]]; then
  verify_checkout "$destination"
  printf 'OpenMinis source ready at %s (%s)\n' "$destination" "$commit"
  exit 0
fi

staging="$repo_root/external/.OpenMinis.provisioning.$$"
[[ ! -e "$staging" ]] || {
  printf 'OpenMinis provisioning staging path already exists: %s\n' "$staging" >&2
  exit 1
}

git init "$staging"
git -C "$staging" remote add origin "$repository_url"
git -C "$staging" sparse-checkout init --no-cone
git -C "$staging" sparse-checkout set --no-cone "${source_paths[@]}"
git -C "$staging" -c http.version=HTTP/1.1 \
  fetch --depth=1 --filter=blob:none origin "$commit"
git -C "$staging" checkout --detach "$commit"
verify_checkout "$staging"
mv "$staging" "$destination"
staging=""
printf 'OpenMinis source provisioned at %s (%s)\n' "$destination" "$commit"
}

run_provision_openminis_source "$@"
