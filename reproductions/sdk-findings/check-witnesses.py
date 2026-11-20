"""Checks every transaction in witnesses.json with py-stellar-base: the
transaction API must parse it, return the network hash and re-encode the
original bytes. Exits 1 on any difference.

Usage: PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/check-witnesses.py
"""
import json
import os
import sys

import stellar_sdk
from stellar_sdk import TransactionBuilder

path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "witnesses.json")
d = json.load(open(path))
bad = 0
for t in d["transactions"]:
    try:
        tx = TransactionBuilder.from_xdr(t["envelope"], d["network_passphrase"])
        problems = [p for p, ok in (("wrong hash", tx.hash_hex() == t["hash"]), ("bytes differ", tx.to_xdr() == t["envelope"])) if not ok]
    except Exception as e:
        problems = [f"refused: {e}"]
    if problems:
        bad += 1
        print(f"{t['shape']}: {', '.join(problems)}")
print(f"py-stellar-base {stellar_sdk.__version__}: {len(d['transactions']) - bad}/{len(d['transactions'])} witnesses ok")
sys.exit(1 if bad else 0)
