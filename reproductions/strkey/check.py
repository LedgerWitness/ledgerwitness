"""Strkey decoder check for reproductions/strkey (Python): prints, for every
vector, whether py-stellar-base's StrKey accepts it and whether it
re-encodes unchanged.

Usage: python3 check.py <vectors.json>
"""
import json, sys
import stellar_sdk
from stellar_sdk import StrKey
fns = {"G": ("decode_ed25519_public_key", "encode_ed25519_public_key"), "M": ("decode_med25519_public_key", "encode_med25519_public_key"),
  "P": ("decode_ed25519_signed_payload", "encode_ed25519_signed_payload"), "B": ("decode_claimable_balance", "encode_claimable_balance"),
  "C": ("decode_contract", "encode_contract"), "W": ("decode_muxed_contract", "encode_muxed_contract"), "L": ("decode_liquidity_pool", "encode_liquidity_pool"),
  "T": ("decode_pre_auth_tx", "encode_pre_auth_tx"), "X": ("decode_sha256_hash", "encode_sha256_hash"), "S": ("decode_ed25519_secret_seed", "encode_ed25519_secret_seed")}
out = []
for v in json.load(open(sys.argv[1])):
    dec, enc = fns.get(v["strkey"][0].upper(), (None, None))
    if not dec or not hasattr(StrKey, dec): r = {"verdict": "unsupported"}
    else:
        try:
            raw = getattr(StrKey, dec)(v["strkey"])
            try: back = getattr(StrKey, enc)(raw)
            except Exception: back = None
            r = {"verdict": "accept", "roundtrip": back == v["strkey"]}
        except Exception as e:
            r = {"verdict": "reject", "error": str(e)[:80]}
    out.append({"strkey": v["strkey"], **r})
print(json.dumps({"sdk": f"python {stellar_sdk.__version__}", "results": out}))
