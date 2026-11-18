# Replay comparison format 0.1.0

Implemented in `crates/replay`, exposed by `ledgerwitness replay-check`. This compares
adapter-supplied normalized observations against an explicit oracle. It does
not launch an indexer, serve RPC, write a database, verify an adapter's claims
about persistence, or implement the future JSON-line adapter lifecycle.

## Scope and identity

Version 0.1.0 covers **operation effects of one focus transaction** in one
verified LCM v1/v2 entry. Exclude fee/sequence bookkeeping. Observations must
come from target storage normalized to this scope; neither envelope inputs
nor a fixture copied into an observation establish persisted behavior.

Each oracle and observation carries `schema_version: "0.1.0"` and `binding`:

| Field | Meaning |
|---|---|
| `entry_id` | Corpus entry ID |
| `manifest_sha256` | SHA-256 of the exact `entry.json` file bytes |
| `artifact_sha256` | SHA-256 of the primary ledger file bytes |
| `ledger_sequence` | Ledger containing the focus transaction, positive u32 |
| `focus_tx_hash` | Focus transaction hash under the manifest's passphrase |

Hashes are 64 lowercase hex characters. The CLI verifies the entry and derives
this binding from current files/header before comparison. A stale oracle or
observation produces `harness_error`. This assumes files remain unchanged
during the command; it is not an atomic filesystem snapshot or attestation.

The output also records hashes of the supplied oracle and observation files.
This binding is specific to replay comparison; existing SDK conformance report
format 0.1.0 does not yet carry equivalent artifact/build digests.

## Oracle assertions

An oracle contains `schema_version`, `binding` and `assertions`. Each assertion
has `kind` and an RFC 6901 `path` into observation `data`:

| Kind | Additional fields | Comparison |
|---|---|---|
| `field` | `value` | Exact JSON equality; string integers are distinct from numbers |
| `absent` | none | Pointer must not exist; a present null value fails |
| `set` | `values`, `mode` | Ignore order/observed duplicates; compare expected members |
| `rows` | `values`, `identity`, `mode` | Objects grouped by identity, retaining row multiplicity |
| `sequence` | `values` | Exact item order and multiplicity; a shorter preserved subsequence is a drop |

`mode` is `exact` (extra values fail) or `subset` (extra values are allowed).
For rows, `identity` is a list of 1–8 JSON pointers relative to each row. Every
identity field must exist and be non-null. Object key insertion order has no
effect. Identical rows match first; unmatched rows with the same identity
are wrong values, missing identities/occurrences are drops. Extra rows fail
only in exact mode. Each row compares in full, including extra object fields;
subset mode permits extra rows, not silently ignored row fields.
Missing identity fields in an observation are malformed
adapter data, not a target-defect verdict.

Expected set values must be unique. Empty exact collections intentionally
assert emptiness; missing collection fields still fail. Empty subsets are
rejected as vacuous. An oracle with no assertions is `inconclusive`.
Unknown versions, assertion kinds and contract fields are rejected; nothing
is silently ignored as an unsupported assertion. Arbitrary field names are
allowed inside assertion values and normalized `data`.

## Observations

An observation contains:

- `schema_version` and `binding` as above;
- `adapter`: nonempty `name`, `version`, `target_revision`, `sdk_version`;
- `last_committed_ledger`: inclusive durable progress, or null/omitted when
  unavailable; fetched/queued sequence and RPC cursors are not valid substitutes;
- `data`: a normalized JSON object with the asserted collections/fields;
- `error`: null/omitted on success, otherwise an adapter error string.

The adapter must translate its target's cursor semantics and read committed
storage after ledger N. Comparison can pass only when supplied progress is
at least N. This check cannot detect an adapter falsely claiming a commit;
the real database adapter and rollback tests remain required.

Use decimal strings for 64-bit integers, lowercase hex for binary values and
full strkeys. The comparator does not interpret arbitrary field names to
infer Stellar types; each oracle/adapter defines those values explicitly.
Unsupported fields must be reported as an adapter error rather than omitted
to manufacture a pass. Optional capabilities and protocol-gap classification
belong to the future adapter/orchestration protocol.

## Verdicts and CLI

| Verdict | Condition | Exit |
|---|---|---|
| `pass` | Committed progress and every assertion matches | 0 |
| `mishandled` | Wrong values or forbidden extra values; takes precedence over drops | 1 |
| `silently_dropped` | Expected values/rows are missing and no wrong/extra values exist | 1 |
| `harness_error` | Stale binding, incomplete adapter identity, adapter error or malformed normalized data | 2 |
| `inconclusive` | No assertions or no evidence of committed progress | 3 |

Input/schema/verification errors also exit 2, without a comparison report.
Comparison reports are printed as JSON and optionally saved with `--json`,
including non-pass reports. Diffs identify missing/extra/wrong values and paths.
They retain at most 128 entries and at most 2048 UTF-8 bytes per value preview;
flags disclose truncation. Previews contain JSON text, not alternative values,
and truncated previews are not necessarily parseable JSON. Verdict computation
continues after the diagnostic cap. Limits are 256 assertions, 4096 items per
collection/sequence and 1024 bytes per pointer; these are toolkit policies.

```sh
cargo run -q -p ledgerwitness -- replay-check \
  --entry corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30/entry.json \
  --oracle corpus/replay/core.testnet.cap73.sac-trust-trustline.p30.json \
  --observed adapter-observation.json --json comparison.json
```

`adapter-observation.json` must be produced by a qualified adapter. None ships
yet. Unit/CLI tests use explicitly synthetic observations and are not claimed
as live integration evidence. Run `cargo test -p ledgerwitness-replay --offline` to
exercise the comparator without an indexer.
