# Conformance probes (version 0.2.0)

A probe is a named value that every conformance runner computes from a corpus
artifact with its own SDK, without reading the expected value first. Runners
are compared with the manifest's `expected_decoded_values.probes` and with each
other. Probes deliberately cover the places where SDKs have diverged before:
strkey encoding of signer and muxed keys, unsigned 64-bit IDs, pool ID
derivation from asset order, and reading state from meta rather than from the
envelope.

## Output encoding

- 32-bit integers are JSON numbers. 64-bit integers are decimal strings.
- Hashes and pool IDs are lowercase hex.
- Accounts, muxed accounts and signer keys are SEP-0023 strkeys
  (`G…`, `M…`, `P…`, `T…`, `X…`).
- Operation types are the XDR constant names from `Stellar-transaction.x`
  (`SET_OPTIONS`, `PAYMENT`, …).
- `null` means "absent" (for example an operation without its own source).

## Ledger-level probes (artifact type `LedgerCloseMeta`, v1 or v2)

| Probe | Definition |
|---|---|
| `ledger_sequence` | `ledger_header.header.ledger_seq` |
| `protocol_version` | `ledger_header.header.ledger_version` |
| `tx_count` | number of entries in `tx_processing` |

## Focus-transaction probes (`focus`)

The focus transaction is the manifest's `focus.tx_hash`. Its probes come from
two artifacts, so that an SDK that cannot decode the whole ledger (for example
an older release meeting a newer header arm) is still tested on the
transaction itself.

**Envelope probes** are computed from the entry's derived `focus_envelope`
artifact (a standalone `TransactionEnvelope`, verified by `ledgerwitness verify` to be
the exact bytes of the focus envelope inside the ledger):

| Probe | Definition |
|---|---|
| `tx_hash` | the SDK's own transaction hash of the envelope under the manifest's network passphrase, hex |
| `tx_source` | source account of the (inner) transaction as a strkey: `M…` if muxed, else `G…` |
| `tx_source_muxed_id` | muxed ID of `tx_source` as a decimal string, or `null` |
| `op_types` | XDR constant name of each operation, in order |
| `op_sources` | each operation's own source account as a strkey, or `null` |
| `muxed_ids` | decimal muxed IDs in this order: transaction source; then for each operation its source, then its destination for `PAYMENT`, `PATH_PAYMENT_STRICT_RECEIVE`, `PATH_PAYMENT_STRICT_SEND` and `ACCOUNT_MERGE`. Non-muxed accounts contribute nothing. |
| `envelope_pool_ids` | for each `CHANGE_TRUST` operation whose line is a pool share: the pool ID the SDK derives from the operation's `LiquidityPoolParameters`, as hex, in operation order. If the SDK's pool-ID function rejects the parameters, the entry is `"error: <message>"`. |

**Meta probes** are computed from the ledger artifact. The runner finds the
`tx_processing` entry whose `result.transaction_hash` equals the focus hash.

| Probe | Definition |
|---|---|
| `successful` | result code is `txSUCCESS`, or `txFEE_BUMP_INNER_SUCCESS` |
| `meta_signer_keys` | sorted, de-duplicated signer keys (strkeys) of every account entry that appears as `created`, `updated` or `restored` in the focus transaction's per-operation meta changes |
| `meta_created_trustlines` | sorted strings `ACCOUNT\|CODE:ISSUER` or `ACCOUNT\|pool:HEX` for every trustline entry `created` in the per-operation meta changes |
| `op_contract_event_counts` | number of contract events recorded for each operation (`TransactionMeta` v4 `operations[i].events`; v3 Soroban meta events for the single operation; 0 otherwise) |

Per-operation meta is used rather than `tx_changes_before`/`tx_changes_after`
so that fee and sequence-number bookkeeping does not appear as operation
effects. `meta_*` probes are order-insensitive on purpose: the order of
ledger-entry changes inside an operation differs between emitters of the same
ledger (see `normalization.md`).

Precise semantics that runners must share:

- `meta_signer_keys` lists the signers of every account the operation touched,
  not only signers that changed. A plain payment from an account that has a
  P… signer therefore reports that signer.
- Asset codes in `meta_created_trustlines` are the code bytes with trailing
  zero bytes removed. stellar-core only accepts ASCII alphanumeric codes, so
  every core-emitted entry renders identically in all runners; runners are not
  required to agree on codes core would reject.
- `op_contract_event_counts` depends on how the emitting node is configured,
  not only on the ledger: classic operations emit CAP-0067 events only when
  the node runs with `EMIT_CLASSIC_EVENTS`. Entries whose expectations rely
  on such events carry the shape tag `emitter:classic_events`.

## Runner result format

Each runner prints one JSON document:

```json
{
  "runner": "rust",
  "library": "stellar-xdr 30.0.0",
  "probe_spec": "0.2.0",
  "results": [
    {
      "id": "core.testnet.cap40.signed-payload-signer.p28",
      "decode": "ok",
      "error": null,
      "roundtrip_exact": true,
      "envelope_decode": "ok",
      "envelope_roundtrip_exact": true,
      "probes": { "...": "..." },
      "inputs": {
        "manifest_sha256": "…",
        "ledger_sha256": "…",
        "envelope_sha256": "…"
      },
      "elapsed_ms": 12.5
    }
  ]
}
```

`decode` (the ledger artifact) and `envelope_decode` (the derived envelope) are
each `ok`, `error` (the library rejected the bytes) or `panic` (the library
crashed). When the ledger does not decode, ledger and meta probes are absent
and the envelope probes are still reported.

When the SDK's own API raises an error on an artifact that decoded, each
affected probe is reported as `{"error": "<message>"}` (or, inside
`envelope_pool_ids`, as an `"error: <message>"` string). `ledgerwitness conform`
classifies that as `rejects_valid_input`. A probe that a runner omits although
its artifact decoded counts as a mismatch: silence is never agreement.

A decoded artifact must report its corresponding `roundtrip_exact` field as
the boolean `true`. Missing, null and non-boolean evidence cannot earn a pass.
When envelope probes are expected, a failed or unreported envelope decode is
a `decode_failure`, even if the ledger decoded or matching probes were
reported. A failed ledger can be `ledger_unsupported` only when the envelope
decoded, round-tripped and matched its probes.

The CLI rejects empty corpora/probe sets, missing reports, unsupported or
missing `probe_spec`, malformed report structure, duplicate report identities,
duplicate entry IDs and result IDs not present in the corpus. An omitted
entry result in an otherwise valid report remains `not_run`.

Before comparison, `ledgerwitness conform` verifies every entry and requires the complete
ledger and focus probe profile above, including exactly one derived focus
envelope. It currently accepts valid LedgerCloseMeta focus fixtures only.
Version 0.2.0 binds each result to its inputs: `inputs` holds the SHA-256 of
the exact manifest, ledger and focus-envelope bytes the runner read. `ledgerwitness
conform` compares them with the entries it has just verified. A result that
claims a decode or reports probes must carry the matching digests, and any
digest that differs means the report was produced from other inputs: the
command refuses it (exit 2), even with `--report-only`. Results that failed
without reading their inputs may omit digests; they can only score as
failures. A digest establishes which inputs a report claims to have used; it
does not prove that the runner ran honestly.

Exit status: with `--require-pass`, only the named libraries gate the result;
with `--report-only`, the command always exits 0 after printing; with
neither, any non-pass cell exits 1.

`--require-pass` requires every named library to be present and all its cells
to pass. Prefixes match the complete library name or a boundary followed by
a space (for example `stellar-xdr` matches `stellar-xdr 30.0.0`). An absent
required library fails even if every supplied library passed. Matrix files
are still written before a required-library failure is reported.
