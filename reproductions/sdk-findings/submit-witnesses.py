"""Submits testnet transactions whose shapes stellar-core accepts but some SDKs
refuse to parse, so that each refusal has an on-chain witness.

Shapes:
  manage-data-space-name    ManageData whose name is one space (0x20)
  nested-not-predicate      CreateClaimableBalance with NOT(NOT(unconditional))
  precond-min-seq-num-neg   PRECOND_V2 whose only field is minSeqNum = -1
  precond-max-ledger-u32    PRECOND_V2 whose only field is ledgerBounds {0, 2^32-1}
  revoke-pool-share-sponsorship
                            RevokeSponsorship of a sponsored pool-share trustline

Accounts are fresh friendbot accounts, discarded after the run. Each shape is
built with py-stellar-base and then edited at the XDR level before signing,
so the SDK's own builder checks cannot alter it. Prints one JSON line per
transaction: shape, hash, ledger, status and the envelope in base64.

Usage: PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/submit-witnesses.py
"""

import base64
import hashlib
import json
import time
import urllib.error
import urllib.parse
import urllib.request

from stellar_sdk import (
    Account,
    Asset,
    ChangeTrust,
    Claimant,
    ClaimPredicate,
    CreateClaimableBalance,
    Keypair,
    LiquidityPoolAsset,
    LiquidityPoolId,
    ManageData,
    Network,
    RevokeSponsorship,
    BeginSponsoringFutureReserves,
    EndSponsoringFutureReserves,
    TransactionBuilder,
)
from stellar_sdk import xdr as X

HORIZON = "https://horizon-testnet.stellar.org"
PASSPHRASE = Network.TESTNET_NETWORK_PASSPHRASE
NETWORK_ID = hashlib.sha256(PASSPHRASE.encode()).digest()


def request(url, data=None):
    # Friendbot refuses urllib's default User-Agent.
    req = urllib.request.Request(url, data=data, headers={"User-Agent": "ledgerwitness-witnesses/0.1"})
    return urllib.request.urlopen(req, timeout=60)


def get(path):
    with request(HORIZON + path) as r:
        return json.load(r)


def fund():
    kp = Keypair.random()
    request("https://friendbot.stellar.org/?addr=" + kp.public_key).read()
    return kp


def sequence(account):
    return int(get("/accounts/" + account)["sequence"])


def build(source, ops, edit=None):
    """A V1 transaction from source with ops; edit(tx_xdr) may change it."""
    seq = sequence(source)
    b = TransactionBuilder(
        source_account=Account(source, seq), network_passphrase=PASSPHRASE, base_fee=1000
    ).add_time_bounds(0, 0)
    for op in ops:
        b.append_operation(op)
    tx = b.build().transaction.to_xdr_object()
    if edit:
        edit(tx)
    return tx


def sign_and_submit(shape, tx, signers):
    payload = X.TransactionSignaturePayload(
        network_id=X.Hash(NETWORK_ID),
        tagged_transaction=X.TransactionSignaturePayloadTaggedTransaction(
            type=X.EnvelopeType.ENVELOPE_TYPE_TX, tx=tx
        ),
    )
    tx_hash = hashlib.sha256(payload.to_xdr_bytes()).digest()
    sigs = [X.DecoratedSignature(X.SignatureHint(kp.signature_hint()), X.Signature(kp.sign(tx_hash))) for kp in signers]
    env = X.TransactionEnvelope(
        type=X.EnvelopeType.ENVELOPE_TYPE_TX, v1=X.TransactionV1Envelope(tx=tx, signatures=sigs)
    )
    b64 = env.to_xdr()
    data = urllib.parse.urlencode({"tx": b64}).encode()
    try:
        with request(HORIZON + "/transactions", data) as r:
            res = json.load(r)
        status, ledger = ("SUCCESS" if res.get("successful") else "FAILED"), res.get("ledger")
    except urllib.error.HTTPError as e:
        res = json.load(e)
        status, ledger = "REJECTED " + json.dumps(res.get("extras", {}).get("result_codes")), None
    out = {"shape": shape, "hash": tx_hash.hex(), "ledger": ledger, "status": status, "envelope": b64}
    print(json.dumps(out), flush=True)
    return out


def set_v2(tx, **fields):
    v2 = X.PreconditionsV2(
        time_bounds=None,
        ledger_bounds=fields.get("ledger_bounds"),
        min_seq_num=fields.get("min_seq_num"),
        min_seq_age=X.Duration(X.Uint64(0)),
        min_seq_ledger_gap=X.Uint32(0),
        extra_signers=[],
    )
    tx.cond = X.Preconditions(type=X.PreconditionType.PRECOND_V2, v2=v2)


def main():
    a = fund()
    b = fund()
    issuer = fund()

    def space_name(tx):
        tx.operations[0].body.manage_data_op.data_name = X.String64(b" ")

    sign_and_submit("manage-data-space-name", build(a.public_key, [ManageData("x", b"v")], space_name), [a])

    pred = ClaimPredicate.predicate_not(ClaimPredicate.predicate_not(ClaimPredicate.predicate_unconditional()))
    sign_and_submit(
        "nested-not-predicate",
        build(a.public_key, [CreateClaimableBalance(Asset.native(), "1", [Claimant(b.public_key, pred)])]),
        [a],
    )

    sign_and_submit(
        "precond-min-seq-num-neg",
        build(a.public_key, [ManageData("lw", b"1")], lambda tx: set_v2(tx, min_seq_num=X.SequenceNumber(X.Int64(-1)))),
        [a],
    )

    bounds = X.LedgerBounds(min_ledger=X.Uint32(0), max_ledger=X.Uint32(2**32 - 1))
    sign_and_submit(
        "precond-max-ledger-u32",
        build(a.public_key, [ManageData("lw", b"2")], lambda tx: set_v2(tx, ledger_bounds=bounds)),
        [a],
    )

    # A sponsors B's pool-share trustline, then revokes the sponsorship.
    lwp = Asset("LWP", issuer.public_key)
    pool = LiquidityPoolAsset(Asset.native(), lwp)
    sign_and_submit("setup-trust-lwp", build(b.public_key, [ChangeTrust(lwp)]), [b])
    sign_and_submit(
        "setup-sponsored-pool-share-trustline",
        build(
            a.public_key,
            [
                BeginSponsoringFutureReserves(b.public_key),
                ChangeTrust(pool, source=b.public_key),
                EndSponsoringFutureReserves(source=b.public_key),
            ],
        ),
        [a, b],
    )
    time.sleep(6)
    sign_and_submit(
        "revoke-pool-share-sponsorship",
        build(a.public_key, [RevokeSponsorship.revoke_trustline_sponsorship(b.public_key, LiquidityPoolId(pool.liquidity_pool_id))]),
        [a],
    )


if __name__ == "__main__":
    main()
