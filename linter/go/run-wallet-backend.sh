#!/usr/bin/env bash
# Runs STLR003 on wallet-backend at the commit before its Soroban-participants
# fix, at the fix, and at a current commit, and checks the expected outcome:
# findings before the fix, none after.
#
# Usage: linter/go/run-wallet-backend.sh <wallet-backend-clone> [current-commit]
#
# Commits (stellar/wallet-backend):
#   pre-fix  666a64d426263fa6bd2a0ee61ee8dfb1f4729019  parent of the fix
#   fix      9538b3ca  "derive Soroban participants from executed meta, not the
#            auth tree" (wallet-backend#753)
#   current  the clone's HEAD unless given
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
clone="$(cd "${1:?wallet-backend clone path}" && pwd)"
current="${2:-$(git -C "$clone" rev-parse HEAD)}"

bin="$(mktemp -d)/ledgerwitness-lint"
(cd "$here" && go build -o "$bin" ./cmd/ledgerwitness-lint)

worktrees=()
cleanup() {
  for wt in "${worktrees[@]}"; do
    git -C "$clone" worktree remove --force "$wt" 2>/dev/null || true
  done
  rm -rf "$(dirname "$bin")"
}
trap cleanup EXIT

# Prints the findings at one commit and returns how many there were.
lint() {
  local sha="$1" wt
  wt="$(mktemp -d)/wallet-backend"
  worktrees+=("$wt")
  git -C "$clone" worktree add --quiet --detach "$wt" "$sha"
  echo "== $(git -C "$clone" rev-parse --short=8 "$sha") ($(git -C "$clone" log -1 --format=%cs "$sha"))" >&2
  # multichecker exits 3 when it reports findings; anything else non-zero is a
  # failure to analyse, not a result.
  local out status=0
  out="$(cd "$wt" && "$bin" -test=false ./internal/... 2>&1)" || status=$?
  if [[ $status -ne 0 && $status -ne 3 ]]; then
    echo "$out" >&2
    echo "error: analysis failed at $sha (exit $status)" >&2
    exit 1
  fi
  echo "${out//$wt\//}" | grep 'STLR003' >&2 || true
  echo "$out" | grep -c 'STLR003' || true
}

before="$(lint 666a64d426263fa6bd2a0ee61ee8dfb1f4729019)"
fix="$(lint 9538b3ca)"
now="$(lint "$current")"

echo "STLR003 findings: $before before the fix, $fix at the fix, $now at $(git -C "$clone" rev-parse --short=8 "$current")"
[[ "$before" -gt 0 && "$fix" -eq 0 && "$now" -eq 0 ]]
