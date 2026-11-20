# Corpus format

This directory is the language-neutral part of the project: raw XDR files and
JSON manifests. Any pipeline or SDK can consume it without the tools in the
rest of the repository.

```
corpus/
  schema/manifest.schema.json        JSON Schema (2020-12) for entry.json
  entries/
    core-emitted/<id>/               emitted by stellar-core, provenance verified
    synthetic-validated/<id>/        constructed, admitted with evidence
    sdk-negative/<id>/               invalid input for SDK validation tests
```

## An entry

```
corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/
  entry.json            manifest
  ledger.xdr            primary artifact (raw XDR, here a LedgerCloseMeta)
  focus-envelope.xdr    derived: the transaction of interest, cut from ledger.xdr
  ledger.rpc.xdr        alternate: the same ledger from another source (optional)
```

All `.xdr` files are raw binary XDR, not base64, of the type named in the
manifest (`xdr_type` for the primary artifact, `derived_artifacts[].xdr_type`
for derived ones).

Artifact paths must name regular files directly inside the entry directory;
symlinks are rejected. An invalid manifest stops verification before artifact
reads. Valid fixture classes must expect `decode_roundtrip_exact`.
`reject_on_validation` is reserved for a future semantic validator and fails
explicitly today. Core-emitted provenance verification currently supports
`LedgerCloseMeta` primary artifacts only. A core-emitted ledger must also
contain at least one transaction and carry a header signature from a pinned
validator of its network (testnet today); otherwise it fails verification.

Entry IDs follow `<class>.<network>.<cap>.<shape>.p<protocol>`, for example
`core.testnet.cap73.sac-trust-trustline.p30`.

## Manifest fields consumers need

| Field | Use |
|---|---|
| `xdr_type`, `xdr_path`, `sha256` | which file to decode as which type; check the hash first |
| `fixture_class` | `core_emitted_valid` and `synthetic_validated` must decode; `sdk_negative_only` follows `expected_outcome` |
| `cap_ids`, `protocol_versions`, `shape_tags` | select entries relevant to your code |
| `network_passphrase` | needed to compute transaction hashes |
| `focus.tx_hash` | the transaction the entry is about, inside a ledger artifact |
| `expected_decoded_values.xdr_json` | JSON-pointer assertions on the SEP-0051 XDR-JSON form |
| `expected_decoded_values.probes` | named values every SDK should derive (definitions in `docs/spec/probes.md`) |
| `expected_state_changes_or_participants` | what an indexer should record for the focus transaction, read from meta |
| `known_regression_ids` | upstream issues the entry guards against |
| `validation_evidence`, `source_url_or_ledger`, `core_version_and_commit` | where the bytes came from and how that was checked |
| `alternate_artifacts[].relation` | how a second copy of the same ledger relates to the primary one |

Value encoding in expected values: 64-bit integers are decimal strings,
binary values are lowercase hex, accounts and keys are SEP-0023 strkeys.

## Minimal consumer check

For each entry: read `entry.json`, check `sha256` of `xdr_path`, decode it as
`xdr_type`, re-encode, and compare the bytes. Then compare whatever your
library exposes against `expected_decoded_values`. `runners/` in this
repository does this for Rust, Go, JavaScript, Python and Java.

## Adding an entry

1. Obtain the artifact (see `generators/` and `ledgerwitness galexie-extract`).
2. Write `entry.json` against the schema; take expected values from what was
   submitted or from an independent source, not from the decoder under test.
3. `cargo run -q -p ledgerwitness -- verify corpus/entries` must pass.
