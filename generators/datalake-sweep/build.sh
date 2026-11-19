#!/usr/bin/env bash
# Builds a cross-SDK sweep set from public data-lake ledgers: one directory per
# transaction, with the ledger, the transaction's envelope and a minimal
# manifest that names them. The manifests carry no expected values: the sweep
# compares the SDK runners with each other (compare.mjs), not with an oracle.
# Sweep directories are never corpus entries.
#
# Usage: generators/datalake-sweep/build.sh <out-dir> <network> <ledger>...
#   network: pubnet or testnet/<reset-date> (a data-lake prefix under
#            v1.1/stellar/ledgers/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
out="${1:?out-dir}"; network="${2:?network}"; shift 2
[[ $# -gt 0 ]] || { echo "usage: build.sh <out-dir> <network> <ledger>..." >&2; exit 2; }
case "$network" in
  pubnet) passphrase="Public Global Stellar Network ; September 2015" ;;
  testnet/*) passphrase="Test SDF Network ; September 2015" ;;
  *) echo "unknown network $network" >&2; exit 2 ;;
esac
lake="https://aws-public-blockchain.s3.us-east-2.amazonaws.com/v1.1/stellar/ledgers/$network"
cargo build -q --release --locked --manifest-path "$repo_root/Cargo.toml" -p ledgerwitness
lw="$repo_root/target/release/ledgerwitness"
mkdir -p "$out"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT

for seq in "$@"; do
  part=$(( seq - seq % 64000 ))
  key="$(printf '%08X--%d-%d/%08X--%d.xdr.zst' $((0xFFFFFFFF - part)) "$part" $((part + 63999)) $((0xFFFFFFFF - seq)) "$seq")"
  curl -fsS "$lake/$key" -o "$work/obj.zst"
  "$lw" galexie-extract "$work/obj.zst" --ledger "$seq" --out "$work/ledger.xdr" >/dev/null
  # One decode writes every envelope as <hash>.xdr.
  rm -rf "$work/envs"
  "$lw" extract-envelope "$work/ledger.xdr" --all-to "$work/envs" \
    --network-passphrase "$passphrase" >/dev/null
  n=0
  for env in "$work"/envs/*.xdr; do
    tx="$(basename "$env" .xdr)"
    dir="$out/sweep.$seq.$tx"
    mkdir -p "$dir"
    ln -f "$work/ledger.xdr" "$dir/ledger.xdr" 2>/dev/null || cp "$work/ledger.xdr" "$dir/ledger.xdr"
    mv "$env" "$dir/focus-envelope.xdr"
    cat > "$dir/entry.json" <<JSON
{"id": "sweep.$seq.$tx", "xdr_type": "LedgerCloseMeta", "xdr_path": "ledger.xdr",
 "network_passphrase": "$passphrase", "focus": {"tx_hash": "$tx"},
 "derived_artifacts": [{"xdr_path": "focus-envelope.xdr", "derived_from": "focus_envelope"}]}
JSON
    n=$((n + 1))
  done
  # Hard links keep one copy of each ledger on disk; the next ledger gets a new file.
  rm -f "$work/ledger.xdr"
  echo "ledger $seq: $n transactions"
done
