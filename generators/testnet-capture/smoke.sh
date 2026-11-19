#!/usr/bin/env bash
# Live testnet check of the proof of concept, end to end:
#   1. capture-testnet.mjs submits the five rare shapes to testnet with
#      throwaway friendbot-funded keys (never written to disk) and saves each
#      resulting LedgerCloseMeta exactly as RPC getLedgers returned it;
#   2. for each capture, the ledger hash is fetched independently from Horizon;
#   3. `ledgerwitness check-ledger` checks the capture against it: header hash,
#      tx-set and result-set commitments, network binding, the validator
#      signature on the header and presence of the submitted transaction;
#   4. sdk-check.mjs runs @stellar/stellar-sdk 17.4.0 and 17.2.0 over each
#      capture and compares what they read with what core recorded in the
#      ledger and with the submitted values. 17.2.0 must still reject the
#      liquidity-pool transaction (js-stellar-sdk#1766).
#
# Needs: npm ci --prefix generators/testnet-capture
#        npm ci --prefix runners/javascript
#
# This writes to testnet. It never touches mainnet.
# Usage: generators/testnet-capture/smoke.sh [out-dir]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
out="${1:-$here/out}"
horizon="${LW_HORIZON_URL:-https://horizon-testnet.stellar.org}"

rm -rf "$out"
mkdir -p "$out"
cargo build -q --locked --manifest-path "$repo_root/Cargo.toml" -p ledgerwitness
ledgerwitness="$repo_root/target/debug/ledgerwitness"

node "$here/capture-testnet.mjs" "$out"

field() { node -e 'console.log(require(process.argv[1])[process.argv[2]])' "$1" "$2"; }

# Horizon can trail RPC by a few ledgers; retry briefly.
horizon_hash() {
  local ledger="$1" body
  for _ in $(seq 1 20); do
    if body="$(curl -fsS "$horizon/ledgers/$ledger" 2>/dev/null)"; then
      node -e 'console.log(JSON.parse(process.argv[1]).hash)' "$body"
      return 0
    fi
    sleep 3
  done
  echo "error: Horizon has no ledger $ledger" >&2
  return 1
}

failed=0
for prov in "$out"/*.provenance.json; do
  shape="$(basename "$prov" .provenance.json)"
  ledger="$(field "$prov" ledger)"
  tx="$(field "$prov" tx_hash)"
  hash="$(horizon_hash "$ledger")"
  echo "== $shape: ledger $ledger, tx $tx"
  if ! "$ledgerwitness" check-ledger "$out/$shape.LedgerCloseMeta.xdr" --tx-hash "$tx" --ledger-hash "$hash"; then
    failed=$((failed + 1))
  fi
done

count="$(ls "$out"/*.provenance.json | wc -l)"
echo
echo "$((count - failed))/$count fresh testnet captures passed check-ledger"

sdk_ok=0
node "$here/sdk-check.mjs" "$out" "$ledgerwitness" --json "$out/sdk-check.json" || sdk_ok=1

[[ "$count" -eq 5 && "$failed" -eq 0 && "$sdk_ok" -eq 0 ]]
