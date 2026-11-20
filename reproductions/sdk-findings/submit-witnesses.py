"""Submits testnet transactions whose shapes stellar-core accepts but some SDKs
refuse to parse, so that each refusal has an on-chain witness.

Shapes:
  manage-data-space-name    ManageData whose name is one space (0x20)
  nested-not-predicate      CreateClaimableBalance with NOT(NOT(unconditional))
  precond-min-seq-num-neg   PRECOND_V2 whose only field is minSeqNum = -1
  precond-max-ledger-u32    PRECOND_V2 whose only field is ledgerBounds {0, 2^32-1}
  revoke-pool-share-sponsorship
                            RevokeSponsorship of a sponsored pool-share trustline

Batch 2 (--batch 2), shapes that model-based SDKs may rebuild differently:
  op-source-equals-tx-source
                            an operation whose source is set to the transaction source
  allow-trust-maintain-liabilities
                            AllowTrust with authorize = 2 (AUTHORIZED_TO_MAINTAIN_LIABILITIES)
  set-trustline-flags-zero  SetTrustLineFlags with clearFlags = setFlags = 0
  fee-bump-muxed-fee-source a fee bump whose fee source is an M... account, ID 2^63+5
  claim-predicate-depth-4   CreateClaimableBalance with 10 claimants, predicates nested 4 deep
  path-payment-5-hops       PathPaymentStrictSend through 5 intermediate assets (fails:
                            no offers; a failed transaction is still in the ledger)
  bump-sequence-zero        BumpSequence with bumpTo = 0
  set-options-empty         SetOptions with no field set

Batch 3 (--batch 3), fields that SDK models convert through their own types:
  create-account-zero-balance
                            a sponsored CreateAccount with startingBalance = 0
  signer-signed-payload-64  SetOptions adding an Ed25519 signed-payload signer, 64-byte payload
  signer-signed-payload-1   the same with a 1-byte payload (strkey padding)
  signer-hash-x             SetOptions adding a sha256-hash (X...) signer
  signer-pre-auth-tx        SetOptions adding a pre-authorized-transaction (T...) signer
  precond-extra-signer-signed-payload-64
                            PRECOND_V2 whose extra signer is a signed payload of 64 bytes,
                            satisfied by a signature of that payload
  lp-deposit-extreme-prices LiquidityPoolDeposit with minPrice 1/2147483647 and maxPrice 2147483647/1
  lp-withdraw-zero-minimums LiquidityPoolWithdraw with both minimum amounts 0

Batch 4 (--batch 4):
  memo-text-not-utf8        a payment whose MEMO_TEXT is the bytes ff fe 20 61 62 63, which are
                            not UTF-8 (stellar-core does not check memo text encoding)

Accounts are fresh friendbot accounts, discarded after the run. Each shape is
built with py-stellar-base and then edited at the XDR level before signing,
so the SDK's own builder checks cannot alter it. Prints one JSON line per
transaction: shape, hash, ledger, status and the envelope in base64.

Usage: PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/submit-witnesses.py [--batch 2|3|4]
"""

import hashlib
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from stellar_sdk import (
    Account,
    AllowTrust,
    Asset,
    BumpSequence,
    ChangeTrust,
    CreateAccount,
    Claimant,
    ClaimPredicate,
    CreateClaimableBalance,
    Keypair,
    LiquidityPoolAsset,
    LiquidityPoolDeposit,
    LiquidityPoolId,
    LiquidityPoolWithdraw,
    ManageData,
    Payment,
    Price,
    Signer,
    SignerKey,
    Network,
    PathPaymentStrictSend,
    RevokeSponsorship,
    SetOptions,
    SetTrustLineFlags,
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


def signatures(tagged, signers):
    payload = X.TransactionSignaturePayload(network_id=X.Hash(NETWORK_ID), tagged_transaction=tagged)
    tx_hash = hashlib.sha256(payload.to_xdr_bytes()).digest()
    sigs = [X.DecoratedSignature(X.SignatureHint(kp.signature_hint()), X.Signature(kp.sign(tx_hash))) for kp in signers]
    return tx_hash, sigs


def v1_envelope(tx, signers):
    tagged = X.TransactionSignaturePayloadTaggedTransaction(type=X.EnvelopeType.ENVELOPE_TYPE_TX, tx=tx)
    tx_hash, sigs = signatures(tagged, signers)
    return tx_hash, X.TransactionEnvelope(type=X.EnvelopeType.ENVELOPE_TYPE_TX, v1=X.TransactionV1Envelope(tx=tx, signatures=sigs))


def sign_and_submit(shape, tx, signers):
    tx_hash, env = v1_envelope(tx, signers)
    return submit(shape, tx_hash, env)


def submit(shape, tx_hash, env):
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


def batch1():
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


def muxed(kp, mid):
    return X.MuxedAccount(
        type=X.CryptoKeyType.KEY_TYPE_MUXED_ED25519,
        med25519=X.MuxedAccountMed25519(id=X.Uint64(mid), ed25519=X.Uint256(kp.raw_public_key())),
    )


def batch2():
    a = fund()
    b = fund()
    issuer = fund()
    lwb = Asset("LWB", issuer.public_key)

    sign_and_submit("op-source-equals-tx-source", build(a.public_key, [ManageData("lw", b"3", source=a.public_key)]), [a])

    # The issuer requires and can revoke authorization; B trusts LWB; the issuer
    # moves B's trustline to "authorized to maintain liabilities".
    sign_and_submit("setup-issuer-auth-flags", build(issuer.public_key, [SetOptions(set_flags=3)]), [issuer])
    sign_and_submit("setup-trust-lwb", build(b.public_key, [ChangeTrust(lwb)]), [b])

    def maintain_liabilities(tx):
        tx.operations[0].body.allow_trust_op.authorize = X.Uint32(2)

    sign_and_submit(
        "allow-trust-maintain-liabilities",
        build(issuer.public_key, [AllowTrust(b.public_key, "LWB", True)], maintain_liabilities),
        [issuer],
    )
    sign_and_submit(
        "set-trustline-flags-zero",
        build(issuer.public_key, [SetTrustLineFlags(b.public_key, lwb, clear_flags=None, set_flags=None)],
              lambda tx: _zero_trustline_flags(tx)),
        [issuer],
    )

    inner_tx = build(a.public_key, [ManageData("lw", b"4")])
    _, inner = v1_envelope(inner_tx, [a])
    fee_bump = X.FeeBumpTransaction(
        fee_source=muxed(b, 2**63 + 5),
        fee=X.Int64(4000),
        inner_tx=X.FeeBumpTransactionInnerTx(type=X.EnvelopeType.ENVELOPE_TYPE_TX, v1=inner.v1),
        ext=X.FeeBumpTransactionExt(0),
    )
    tagged = X.TransactionSignaturePayloadTaggedTransaction(type=X.EnvelopeType.ENVELOPE_TYPE_TX_FEE_BUMP, fee_bump=fee_bump)
    fb_hash, sigs = signatures(tagged, [b])
    submit(
        "fee-bump-muxed-fee-source",
        fb_hash,
        X.TransactionEnvelope(
            type=X.EnvelopeType.ENVELOPE_TYPE_TX_FEE_BUMP,
            fee_bump=X.FeeBumpTransactionEnvelope(tx=fee_bump, signatures=sigs),
        ),
    )

    P = ClaimPredicate
    deep = P.predicate_and(
        P.predicate_or(P.predicate_not(P.predicate_before_absolute_time(4102444800)), P.predicate_before_relative_time(86400)),
        P.predicate_unconditional(),
    )
    claimants = [Claimant(fund().public_key if i < 1 else Keypair.random().public_key, deep) for i in range(10)]
    sign_and_submit("claim-predicate-depth-4", build(a.public_key, [CreateClaimableBalance(Asset.native(), "1", claimants)]), [a])

    path = [Asset(f"LWH{i}", issuer.public_key) for i in range(5)]
    sign_and_submit(
        "path-payment-5-hops",
        build(a.public_key, [PathPaymentStrictSend(b.public_key, Asset.native(), "1", lwb, "1", path)]),
        [a],
    )
    sign_and_submit("bump-sequence-zero", build(a.public_key, [BumpSequence(0)]), [a])

    def empty_set_options(tx):
        op = tx.operations[0].body.set_options_op
        for f in ("inflation_dest", "clear_flags", "set_flags", "master_weight", "low_threshold",
                  "med_threshold", "high_threshold", "home_domain", "signer"):
            setattr(op, f, None)

    sign_and_submit("set-options-empty", build(a.public_key, [SetOptions(home_domain="x")], empty_set_options), [a])


def batch3():
    a = fund()
    b = fund()
    issuer = fund()

    fresh = Keypair.random()

    def zero_balance(tx):
        tx.operations[1].body.create_account_op.starting_balance = X.Int64(0)

    sign_and_submit(
        "create-account-zero-balance",
        build(
            a.public_key,
            [
                BeginSponsoringFutureReserves(fresh.public_key),
                CreateAccount(fresh.public_key, "1"),
                EndSponsoringFutureReserves(source=fresh.public_key),
            ],
            zero_balance,
        ),
        [a, fresh],
    )

    payload64 = bytes(range(1, 65))
    sign_and_submit(
        "signer-signed-payload-64",
        build(a.public_key, [SetOptions(signer=Signer(SignerKey.ed25519_signed_payload(
            _signed_payload(b.public_key, payload64)), 1))]),
        [a],
    )
    sign_and_submit(
        "signer-signed-payload-1",
        build(a.public_key, [SetOptions(signer=Signer(SignerKey.ed25519_signed_payload(
            _signed_payload(b.public_key, b"\x01")), 1))]),
        [a],
    )
    sign_and_submit("signer-hash-x", build(a.public_key, [SetOptions(signer=Signer(SignerKey.sha256_hash(bytes(32 * [7])), 1))]), [a])
    sign_and_submit("signer-pre-auth-tx", build(a.public_key, [SetOptions(signer=Signer(SignerKey.pre_auth_tx(bytes(32 * [9])), 1))]), [a])

    # The extra signer requires a signature by b's key of the 64-byte payload;
    # its hint is the key's last 4 bytes XOR the payload's last 4 bytes.
    def extra_signer(tx):
        sp = X.SignerKeyEd25519SignedPayload(ed25519=X.Uint256(b.raw_public_key()), payload=payload64)
        set_v2(tx)
        tx.cond.v2.extra_signers = [X.SignerKey(type=X.SignerKeyType.SIGNER_KEY_TYPE_ED25519_SIGNED_PAYLOAD, ed25519_signed_payload=sp)]

    tx = build(a.public_key, [ManageData("lw", b"5")], extra_signer)
    tx_hash, env = v1_envelope(tx, [a])
    hint = bytes(x ^ y for x, y in zip(b.raw_public_key()[-4:], payload64[-4:]))
    env.v1.signatures.append(X.DecoratedSignature(X.SignatureHint(hint), X.Signature(b.sign(payload64))))
    submit("precond-extra-signer-signed-payload-64", tx_hash, env)

    # A pool between native and LWL with liquidity from b.
    lwl = Asset("LWL", issuer.public_key)
    pool = LiquidityPoolAsset(Asset.native(), lwl)
    sign_and_submit("setup-trust-lwl", build(b.public_key, [ChangeTrust(lwl)]), [b])
    sign_and_submit("setup-fund-lwl", build(issuer.public_key, [Payment(b.public_key, lwl, "1000")]), [issuer])
    sign_and_submit("setup-trust-pool", build(b.public_key, [ChangeTrust(pool)]), [b])
    sign_and_submit(
        "lp-deposit-extreme-prices",
        build(b.public_key, [LiquidityPoolDeposit(pool.liquidity_pool_id, "100", "100", Price(1, 2147483647), Price(2147483647, 1))]),
        [b],
    )
    sign_and_submit(
        "lp-withdraw-zero-minimums",
        build(b.public_key, [LiquidityPoolWithdraw(pool.liquidity_pool_id, "1", "0", "0")]),
        [b],
    )


def batch4():
    a = fund()
    b = fund()

    def memo_bytes(tx):
        tx.memo = X.Memo(type=X.MemoType.MEMO_TEXT, text=b"\xff\xfe abc")

    sign_and_submit("memo-text-not-utf8", build(a.public_key, [Payment(b.public_key, Asset.native(), "1")], memo_bytes), [a])


def _signed_payload(account, payload):
    from stellar_sdk import SignedPayloadSigner

    return SignedPayloadSigner(account, payload)


def _zero_trustline_flags(tx):
    op = tx.operations[0].body.set_trust_line_flags_op
    op.clear_flags = X.Uint32(0)
    op.set_flags = X.Uint32(0)


if __name__ == "__main__":
    batch = sys.argv[sys.argv.index("--batch") + 1] if "--batch" in sys.argv else "1"
    {"1": batch1, "2": batch2, "3": batch3, "4": batch4}[batch]()
