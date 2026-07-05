#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

readonly -a exclude_globs=(
  "--glob=!target/**"
  "--glob=!dist/**"
  "--glob=!artifacts/**"
  "--glob=!docs/wiki/**"
  "--glob=!.mempalace/**"
  "--glob=!memory/*-mempalace-corpus/**"
  "--glob=!test-palaces/**"
  "--glob=!tmp/**"
  "--glob=!**/build/**"
  "--glob=!**/.gradle/**"
  "--glob=!**/node_modules/**"
  "--glob=!**/coverage/**"
)

for arg in "$@"; do
  case "$arg" in
    --no-ignore|--no-ignore=*|--no-ignore-vcs|--no-ignore-vcs=*|--no-ignore-parent|--no-ignore-parent=*|--no-ignore-global|--no-ignore-global=*|--ignore-file|--ignore-file=*|-u|-uu|-uuu|--unrestricted)
      printf 'source-search: refusing unsafe rg option `%s`\n' "$arg" >&2
      exit 2
      ;;
  esac
done

readonly -a candidate_roots=(
  "AGENTS.md"
  "Cargo.toml"
  "Makefile"
  "apps"
  "crates"
  "xtask"
  "scripts"
  "docs/architecture"
  "docs/config"
  "docs/debug"
  "docs/design"
  "docs/function-maps"
  "docs/goals"
  "docs/loops"
  "docs/mainline-calls"
  "docs/references"
  "docs/release.md"
  "docs/runtime"
  "docs/testing"
  ".agents"
  ".github"
  ".githooks"
)

search_roots=()
for root in "${candidate_roots[@]}"; do
  if [[ -e "$root" ]]; then
    search_roots+=("$root")
  fi
done

# Keep hard excludes after caller-provided args so generated/runtime globs cannot be re-included.
exec rg --hidden "$@" "${exclude_globs[@]}" "${search_roots[@]}"
