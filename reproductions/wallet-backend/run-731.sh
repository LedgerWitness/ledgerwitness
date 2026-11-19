#!/usr/bin/env bash
# Replays the corpus CAP-0073 ledger (a trustline created by the Stellar Asset
# Contract's trust function) through wallet-backend before and after the fix
# for wallet-backend#731, and records what each commit's indexer produced.
#
# Usage: reproductions/wallet-backend/run-731.sh <wallet-backend-clone> [out-dir]
#
# Commits (from `gh api repos/stellar/wallet-backend/pulls/734`):
#   pre-fix  764a4f5b  first parent of the #734 merge
#   post-fix c152783c  merge of #734, "fix: record SAC-created trustline state changes"
#
# Both commits pin go-stellar-sdk v0.7.3, which predates protocol 30, and the
# ledger is protocol 30. Each commit therefore runs twice: with its original
# pin, and with go-stellar-sdk raised to the version our Go runner uses. The
# raise is the same at both commits, so the only difference between the two
# raised runs is wallet-backend's own code.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
clone="$(cd "${1:?wallet-backend clone path}" && pwd)"
out="${2:-$here/results}"
mkdir -p "$out"

entry="$repo_root/corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30"
ledger="$entry/ledger.xdr"
passphrase="Test SDF Network ; September 2015"
sdk="$(awk '$1 == "require" && $2 ~ /go-stellar-sdk/ {print $3}' "$repo_root/runners/go/go.mod")"

worktrees=()
cleanup() {
  for wt in "${worktrees[@]}"; do
    git -C "$clone" worktree remove --force "$wt" 2>/dev/null || true
    rm -rf "$(dirname "$wt")"
  done
}
trap cleanup EXIT

run() {
  local name="$1" sha="$2" raise="$3"
  local wt report="$out/$name.json" log="$out/$name.log"
  rm -f "$report" "$log"
  wt="$(mktemp -d)/wallet-backend-$name"
  worktrees+=("$wt")
  git -C "$clone" worktree add --quiet --detach --force "$wt" "$sha"
  cp "$here/harness-731/ledgerwitness_trustline_test.go" "$wt/internal/indexer/"
  echo "== $name ($sha)"
  (
    cd "$wt"
    if [[ "$raise" == yes ]]; then
      go get "github.com/stellar/go-stellar-sdk@$sdk" >/dev/null 2>&1
      go mod tidy >/dev/null 2>&1
    fi
    echo "go-stellar-sdk $(go list -m -f '{{.Version}}' github.com/stellar/go-stellar-sdk)"
    LW_LEDGER="$ledger" LW_PASSPHRASE="$passphrase" LW_REPORT="$report" \
      go test ./internal/indexer -run '^TestLWTrustlines$' -count=1 2>&1 | tail -4
  ) | tee "$log" || true # a failing run is a result to record, checked below
  if [[ -s "$report" ]]; then
    node -e '
      const r = require(process.argv[1]);
      console.log(`created in meta: ${r.trustlines_created_in_meta?.length ?? 0}, ` +
        `TRUSTLINE state changes: ${r.trustline_state_changes?.length ?? 0}, ` +
        `created without TRUSTLINE/ADD: ${r.created_without_trustline_add?.length ?? 0}`);
    ' "$report" | tee -a "$log"
  else
    echo "no report: the harness did not complete" | tee -a "$log"
  fi
}

run pre-fix-731-original-pin 764a4f5b no
run post-fix-731-original-pin c152783c no
run pre-fix-731 764a4f5b yes
run post-fix-731 c152783c yes
node "$here/check-results-731.mjs" "$out/pre-fix-731.json" "$out/post-fix-731.json" "$entry/entry.json"
