#!/usr/bin/env bash
# Puts the signed-payload signer of the #729 ledger through wallet-backend's
# signer column at three commits, for the third producer named in
# wallet-backend#716, and records what each stores and reads back.
#
# Usage: reproductions/wallet-backend/run-716.sh <wallet-backend-clone> [out-dir]
#
# Commits:
#   pre-fix   79ea2acd  first parent of #693: Value() keeps 32 payload bytes
#   guard     6e5a5910  merge of #693, "reduce muxed addresses to their base
#                       account": Value() refuses longer payloads
#   fixed     2c873353  merge of #729: a signer key column that stores the
#                       whole key
# All three pin go-stellar-sdk v0.7.3, which decodes this protocol-28 ledger,
# so no dependency is changed.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
clone="$(cd "${1:?wallet-backend clone path}" && pwd)"
out="${2:-$here/results}"
mkdir -p "$out"

entry="$repo_root/corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28"
ledger="$entry/ledger.xdr"
passphrase="Test SDF Network ; September 2015"

worktrees=()
cleanup() {
  for wt in "${worktrees[@]}"; do
    git -C "$clone" worktree remove --force "$wt" 2>/dev/null || true
    rm -rf "$(dirname "$wt")"
  done
}
trap cleanup EXIT

run() {
  local name="$1" sha="$2"
  local wt report="$out/$name.json"
  rm -f "$report"
  wt="$(mktemp -d)/wallet-backend-$name"
  worktrees+=("$wt")
  git -C "$clone" worktree add --quiet --detach --force "$wt" "$sha"
  cp "$here/harness-716/ledgerwitness_roundtrip_test.go" "$wt/internal/indexer/"
  echo "== $name ($sha)"
  (cd "$wt" && LW_LEDGER="$ledger" LW_PASSPHRASE="$passphrase" LW_REPORT="$report" \
    go test ./internal/indexer -run '^TestLWSignerRoundTrip$' -count=1 2>&1 | tail -3) || true
  if [[ ! -s "$report" ]]; then
    echo "error: $name produced no report; the harness did not run" >&2
    exit 1
  fi
  node -e '
    for (const s of require(process.argv[1]).signer_round_trips ?? [])
      console.log(`${s.column_type}: ` + (s.write_error ? `write refused: ${s.write_error}`
        : `stored ${s.stored_bytes} bytes, read back ${s.read_back}` +
          (s.read_back_decode_error ? ` (undecodable: ${s.read_back_decode_error})` : "") +
          (s.exact ? " (exact)" : "")));
  ' "$report"
}

run pre-fix-716 79ea2acd1dc65a07166133cc04b12e76db314db0
run guard-716 6e5a59101158dc2996295450190142366062cd05
run fixed-716 2c873353c1f0f3c9b145e25a7d009023ad104a1c
node "$here/check-results-716.mjs" "$out/pre-fix-716.json" "$out/guard-716.json" "$out/fixed-716.json" "$entry/entry.json"
