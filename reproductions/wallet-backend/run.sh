#!/usr/bin/env bash
# Replays a corpus ledger through wallet-backend at the commit before a fix and
# at the fix commit, and records what each does with it.
#
# Usage: reproductions/wallet-backend/run.sh <wallet-backend-clone> [out-dir]
#
# Commits (from `gh api repos/stellar/wallet-backend/pulls/729`):
#   pre-fix  2a0a84ea12523b6c92e278c0c84b443041240095  first parent of the #729 merge
#   post-fix 2c873353c1f0f3c9b145e25a7d009023ad104a1c  merge of #729
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
clone="$(cd "${1:?wallet-backend clone path}" && pwd)"
out="${2:-$here/results}"
mkdir -p "$out"

ledger="$repo_root/corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/ledger.xdr"
passphrase="Test SDF Network ; September 2015"

# Worktrees created by this run, removed on any exit.
worktrees=()
cleanup() {
  for wt in "${worktrees[@]}"; do
    git -C "$clone" worktree remove --force "$wt" 2>/dev/null || true
    rm -rf "$(dirname "$wt")"
  done
}
trap cleanup EXIT

run() {
  local name="$1" sha="$2" shim="$3"
  local wt report="$out/$name.json"
  # A stale report from an earlier run must never be certified as fresh.
  rm -f "$report"
  wt="$(mktemp -d)/wallet-backend-$name"
  worktrees+=("$wt")
  git -C "$clone" worktree add --quiet --detach --force "$wt" "$sha"
  cp "$here/harness/ledgerwitness_replay_test.go" "$wt/internal/data/"
  cp "$here/shims/$shim/export_ledgerwitness_test.go" "$wt/internal/data/"
  echo "== $name ($sha)"
  (cd "$wt" && LW_LEDGER="$ledger" LW_PASSPHRASE="$passphrase" LW_REPORT="$report" \
    go test ./internal/data -run '^TestLWReplay$' -count=1 -v 2>&1 | tail -5)
  # go test exits 0 when the test is skipped or matches nothing; the report
  # is the proof that the harness ran.
  if [[ ! -s "$report" ]]; then
    echo "error: $name produced no report; the harness did not run" >&2
    exit 1
  fi
  cat "$report"
}

run pre-fix-729 2a0a84ea12523b6c92e278c0c84b443041240095 pre-fix-729
run post-fix-729 2c873353c1f0f3c9b145e25a7d009023ad104a1c post-fix-729
node "$here/check-results.mjs" "$out/pre-fix-729.json" "$out/post-fix-729.json"
