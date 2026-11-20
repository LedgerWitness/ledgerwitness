"""Python conformance runner: computes the probes in docs/spec/probes.md for
every corpus entry with py-stellar-base (the stellar-sdk package on PyPI).

Envelope probes go through the SDK's transaction API
(TransactionBuilder.from_xdr and its parsed operations), the layer
applications use; meta probes read the SDK's decoded XDR objects. The logic
mirrors runners/javascript/runner.mjs so that a difference reflects the SDK.

Usage: python3 runner.py <corpus-entries-dir>
"""

import base64
import hashlib
import json
import os
import sys
import time

import stellar_sdk
from stellar_sdk import FeeBumpTransactionEnvelope, LiquidityPoolAsset, SignerKey, StrKey, TransactionBuilder
from stellar_sdk import xdr as X

LIBRARY = f"py-stellar-base {stellar_sdk.__version__}"


def find_entries(root):
    out = []
    for d, _, files in os.walk(root):
        if "entry.json" in files:
            out.append(os.path.join(d, "entry.json"))
    return sorted(out)


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def strkey(muxed):
    """M... if the account is muxed, else G..."""
    return muxed.account_muxed or muxed.account_id


def muxed_id(muxed):
    return None if muxed is None or muxed.account_muxed_id is None else str(muxed.account_muxed_id)


def account_strkey(account_id):
    return StrKey.encode_ed25519_public_key(account_id.account_id.ed25519.uint256)


def asset_code(code_bytes):
    return code_bytes.rstrip(b"\0").decode("latin-1")


def error_probe(e):
    return {"error": str(e)[:200]}


def envelope_probes(raw, passphrase):
    env = TransactionBuilder.from_xdr(base64.b64encode(raw).decode(), passphrase)
    tx_hash = env.hash_hex()
    if isinstance(env, FeeBumpTransactionEnvelope):
        env = env.transaction.inner_transaction_envelope
    tx = env.transaction
    ids = []

    def push(m):
        i = muxed_id(m)
        if i is not None:
            ids.append(i)

    push(tx.source)
    pools = []
    for op in tx.operations:
        push(op.source)
        if op.to_xdr_object().body.type in (
            X.OperationType.PAYMENT,
            X.OperationType.PATH_PAYMENT_STRICT_RECEIVE,
            X.OperationType.PATH_PAYMENT_STRICT_SEND,
            X.OperationType.ACCOUNT_MERGE,
        ):
            push(op.destination)
        if op.to_xdr_object().body.type == X.OperationType.CHANGE_TRUST and isinstance(op.asset, LiquidityPoolAsset):
            try:
                pools.append(op.asset.liquidity_pool_id)
            except Exception as e:  # the SDK refused the pool parameters
                pools.append(f"error: {e}")
    return {
        "tx_hash": tx_hash,
        "tx_source": strkey(tx.source),
        "tx_source_muxed_id": muxed_id(tx.source),
        "op_types": [op.to_xdr_object().body.type.name for op in tx.operations],
        "op_sources": [strkey(op.source) if op.source else None for op in tx.operations],
        "muxed_ids": ids,
        "envelope_pool_ids": pools,
    }


def operation_meta(meta):
    if meta.v == 4:
        return [(o.changes.ledger_entry_changes, len(o.events)) for o in meta.v4.operations]
    if meta.v == 3:
        ops = meta.v3.operations
        n = len(meta.v3.soroban_meta.events) if meta.v3.soroban_meta and len(ops) == 1 else 0
        return [(o.changes.ledger_entry_changes, n) for o in ops]
    body = {2: meta.v2, 1: meta.v1}.get(meta.v)
    ops = body.operations if body is not None else meta.operations
    return [(o.changes.ledger_entry_changes, 0) for o in ops]


def trustline_asset(a):
    if a.type == X.AssetType.ASSET_TYPE_POOL_SHARE:
        return f"pool:{a.liquidity_pool_id.pool_id.hash.hex()}"
    if a.type == X.AssetType.ASSET_TYPE_CREDIT_ALPHANUM4:
        return f"{asset_code(a.alpha_num4.asset_code.asset_code4)}:{account_strkey(a.alpha_num4.issuer)}"
    if a.type == X.AssetType.ASSET_TYPE_CREDIT_ALPHANUM12:
        return f"{asset_code(a.alpha_num12.asset_code.asset_code12)}:{account_strkey(a.alpha_num12.issuer)}"
    return "native"


def ledger_probes(lcm, focus_hash):
    m = {0: lcm.v0, 1: lcm.v1, 2: lcm.v2}[lcm.v]
    ledger = {
        "ledger_sequence": m.ledger_header.header.ledger_seq.uint32,
        "protocol_version": m.ledger_header.header.ledger_version.uint32,
        "tx_count": len(m.tx_processing),
    }
    if not focus_hash:
        return ledger, {}
    tp = next((p for p in m.tx_processing if p.result.transaction_hash.hash.hex() == focus_hash), None)
    if tp is None:
        raise ValueError("focus transaction not in tx_processing")
    code = tp.result.result.result.code
    signers, trustlines, events = set(), [], []
    for changes, n in operation_meta(tp.tx_apply_processing):
        events.append(n)
        for c in changes:
            entry = c.created or c.updated or c.restored
            if entry is None:
                continue
            d = entry.data
            if d.type == X.LedgerEntryType.ACCOUNT:
                for s in d.account.signers:
                    signers.add(SignerKey.from_xdr_object(s.key).encoded_signer_key)
            elif d.type == X.LedgerEntryType.TRUSTLINE and c.created is not None:
                trustlines.append(f"{account_strkey(d.trust_line.account_id)}|{trustline_asset(d.trust_line.asset)}")
    return ledger, {
        "successful": code in (X.TransactionResultCode.txSUCCESS, X.TransactionResultCode.txFEE_BUMP_INNER_SUCCESS),
        "meta_signer_keys": sorted(signers),
        "meta_created_trustlines": sorted(trustlines),
        "op_contract_event_counts": events,
    }


_roundtrips = {}


def _roundtrip(lcm, raw):
    key = sha256(raw)
    if key not in _roundtrips:
        _roundtrips.clear()
        _roundtrips[key] = lcm.to_xdr_bytes() == raw
    return _roundtrips[key]


# Decoded ledgers by SHA-256: sweep sets share one ledger among many
# transactions, and pure-Python decoding of a large ledger is slow.
_decoded = {}


def decode_ledger(raw):
    key = sha256(raw)
    if key not in _decoded:
        _decoded.clear()
        _decoded[key] = X.LedgerCloseMeta.from_xdr_bytes(raw)
    return _decoded[key]


def run_entry(path):
    manifest_bytes = open(path, "rb").read()
    entry = json.loads(manifest_bytes)
    inputs = {"manifest_sha256": sha256(manifest_bytes)}
    d = os.path.dirname(path)
    started = time.perf_counter()
    result = {
        "id": entry["id"],
        "decode": None,
        "error": None,
        "roundtrip_exact": None,
        "envelope_decode": None,
        "envelope_roundtrip_exact": None,
        "probes": {},
    }
    errors, focus = [], {}

    if entry.get("xdr_type") == "LedgerCloseMeta":
        raw = open(os.path.join(d, entry["xdr_path"]), "rb").read()
        inputs["ledger_sha256"] = sha256(raw)
        lcm = None
        try:
            lcm = decode_ledger(raw)
            result["decode"] = "ok"
        except (RecursionError, MemoryError) as e:
            result["decode"] = "panic"
            errors.append(f"ledger: {e}")
        except Exception as e:
            result["decode"] = "error"
            errors.append(f"ledger: {e}")
        if lcm is not None:
            result["roundtrip_exact"] = _roundtrip(lcm, raw)
            try:
                ledger, meta = ledger_probes(lcm, (entry.get("focus") or {}).get("tx_hash"))
                result["probes"].update(ledger)
                focus.update(meta)
            except Exception as e:
                errors.append(f"ledger probes: {e}")

    derived = next((a for a in entry.get("derived_artifacts", []) if a.get("derived_from") == "focus_envelope"), None)
    if derived:
        raw = open(os.path.join(d, derived["xdr_path"]), "rb").read()
        inputs["envelope_sha256"] = sha256(raw)
        try:
            env = X.TransactionEnvelope.from_xdr_bytes(raw)
            result["envelope_decode"] = "ok"
            result["envelope_roundtrip_exact"] = env.to_xdr_bytes() == raw
        except Exception as e:
            result["envelope_decode"] = "error"
            errors.append(f"envelope: {e}")
        if result["envelope_decode"] == "ok":
            try:
                focus.update(envelope_probes(raw, entry["network_passphrase"]))
            except Exception as e:
                # The raw XDR decoded but the SDK's transaction API rejected it.
                errors.append(f"envelope probes: {e}")
                for k in ("tx_hash", "tx_source", "tx_source_muxed_id", "op_types", "op_sources", "muxed_ids", "envelope_pool_ids"):
                    focus[k] = error_probe(e)
    if focus:
        result["probes"]["focus"] = focus
    if errors:
        result["error"] = "; ".join(errors)
    result["inputs"] = inputs
    result["elapsed_ms"] = round((time.perf_counter() - started) * 1000, 1)
    return result


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "corpus/entries"
    results = []
    for p in find_entries(root):
        try:
            results.append(run_entry(p))
        except Exception as e:
            # Report under the entry's own ID so the matrix can score it.
            try:
                rid = json.load(open(p))["id"]
            except Exception:
                rid = os.path.basename(os.path.dirname(p))
            results.append({
                "id": rid, "decode": "panic", "error": str(e), "roundtrip_exact": None,
                "envelope_decode": None, "envelope_roundtrip_exact": None, "probes": {},
            })
    json.dump({"runner": "python", "library": LIBRARY, "probe_spec": "0.2.0", "results": results}, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
