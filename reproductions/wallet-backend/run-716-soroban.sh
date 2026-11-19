#!/usr/bin/env bash
# Replays the corpus ExtendFootprintTTL entry, whose operation source is a
# muxed M... account, through wallet-backend at three commits, for the first
# producer named in wallet-backend#716 (a Soroban operation's muxed source
# seeded as a participant), and records what the participant column stores
# and reads back.
#
# Usage: reproductions/wallet-backend/run-716-soroban.sh <wallet-backend-clone> [out-dir]
#
# Commits:
#   pre-fix  79ea2acd  first parent of #693: AddressBytea keeps 32 payload bytes
#   guard    6e5a5910  merge of #693: AddressBytea refuses longer payloads
#   fixed    bb0b162b  "reduce a Soroban operation's source to its base account"
#
# All three pin go-stellar-sdk v0.7.3, which predates protocol 30, and the
# ledger is protocol 30. Each commit runs with its original pin (recorded) and
# then with go-stellar-sdk raised to the Go runner's version, the same change
# at all three.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
clone="$(cd "${1:?wallet-backend clone path}" && pwd)"
out="${2:-$here/results}"
mkdir -p "$out"

entry="$repo_root/corpus/entries/core-emitted/core.testnet.cap27.soroban-muxed-extend-ttl.p30"
ledger="$entry/ledger.xdr"
passphrase="Test SDF Network ; September 2015"
account="$(node -p "require('$entry/entry.json').expected_decoded_values.probes.focus.tx_source")"
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
  cp "$here/harness-716-soroban/ledgerwitness_participants_test.go" "$wt/internal/indexer/"
  echo "== $name ($sha)"
  (
    cd "$wt"
    if [[ "$raise" == yes ]]; then
      go get "github.com/stellar/go-stellar-sdk@$sdk" >/dev/null 2>&1
      go mod tidy >/dev/null 2>&1
    fi
    echo "go-stellar-sdk $(go list -m -f '{{.Version}}' github.com/stellar/go-stellar-sdk)"
    LW_LEDGER="$ledger" LW_PASSPHRASE="$passphrase" LW_ACCOUNT="$account" LW_REPORT="$report" \
      go test ./internal/indexer -run '^TestLWParticipantRoundTrip$' -count=1 2>&1 | tail -4
  ) | tee "$log" || true # a failing run is a result to record, checked below
  if [[ -s "$report" ]]; then
    node -e '
      for (const p of require(process.argv[1]).operation_participants ?? [])
        console.log(`${p.participant}: ` + (p.write_error ? `write refused: ${p.write_error}`
          : `stored ${p.stored_bytes} bytes, read back ${p.read_back}` +
            (p.read_back_decode_error ? ` (undecodable: ${p.read_back_decode_error})` : "") +
            (p.exact ? " (exact)" : "")));
    ' "$report" | tee -a "$log"
  else
    echo "no report: the harness did not complete" | tee -a "$log"
  fi
}

run pre-fix-716-soroban-original-pin 79ea2acd1dc65a07166133cc04b12e76db314db0 no
run pre-fix-716-soroban 79ea2acd1dc65a07166133cc04b12e76db314db0 yes
run guard-716-soroban 6e5a59101158dc2996295450190142366062cd05 yes
run fixed-716-soroban bb0b162b yes
node "$here/check-results-716-soroban.mjs" \
  "$out/pre-fix-716-soroban.json" "$out/guard-716-soroban.json" "$out/fixed-716-soroban.json" "$entry/entry.json"
