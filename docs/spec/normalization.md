# LedgerCloseMeta equivalence and normalization

A ledger is identified by its header hash, and the header commits to the
transaction set and the transaction results. The ledger-entry changes, events
and diagnostics in `LedgerCloseMeta` are not committed to by anything in the
header. Two stellar-core processes that close the same ledger can therefore
emit `LedgerCloseMeta` that differ in bytes, and both are genuine core output.
This document records what the corpus treats as "the same ledger" and why.

## What was observed

The protocol-30 testnet ledgers in `corpus/entries/core-emitted/` were each
fetched twice: from the public Galexie data lake (`aws-public-blockchain`,
`v1.1/stellar/ledgers/testnet/`) and from the testnet RPC `getLedgers` method.
`ledgerwitness compare-lcm` on each pair reports:

| Ledger | Byte-identical | Equal under core normalization | Equal under core normalization, ext ignored |
|---|---|---|---|
| 5727135 | no | no | yes |
| 5727136 | no | no | yes |
| 5727137 | no | no | yes |
| 5727139 | no | no | yes |
| 5818357 | no | no | yes |

Breaking the differences down (counts are for the first four ledgers;
5818357, added later, differs in the same three ways):

1. **LedgerCloseMeta extension.** The data-lake copies carry `ext` v1
   (`soroban_fee_write1_kb = 1028`); the RPC copies carry `ext` v0. This is the
   whole 12-byte size difference.
2. **Order of ledger-entry changes within an operation.** 28 of the 76
   operations in these ledgers list the same changes in a different order. No
   operation's set of changes differs. In both sources every `state` entry is
   immediately followed by its own `updated` entry (359 of 359 pairs), so
   pairing is preserved; what moves is the order of the pairs and of `created`
   entries.
3. **Diagnostic events.** Only the u64 payload of `core_metrics` /
   `invoke_time_nsecs` timing events differs.

Transaction results, contract events, transaction events, the header, the
transaction set, SCP info and evicted keys are identical.

## The normalization

stellar-core's own `--check-lcm` mode (stellar-core#5435) compares golden
LedgerCloseMeta with `lcmSemanticallyEqual` in `src/test/test.cpp`, which
applies `normalizeMeta` and `zeroNonDeterministicDiagnostics` from
`src/util/MetaUtils.cpp` to both sides. `crates/ledgerwitness/src/normalize.rs` ports
those two functions:

- every `LedgerEntryChanges` list core normalizes (upgrade changes, fee
  processing, `tx_changes_before`/`tx_changes_after`, per-operation changes) is
  sorted by (ledger key, change type ranked STATE < CREATED < UPDATED <
  REMOVED < RESTORED, SHA-256 of the change);
- the u64 data of `core_metrics`/`invoke_time_nsecs` diagnostic events is set
  to zero.

Like core, it leaves `post_tx_apply_fee_processing`, events and the LCM
extension alone. Core orders ledger keys with xdrpp's structural comparison;
the port orders them by XDR bytes. The resulting order can differ, but the
equality relation is the same, because both orders are total (the hash breaks
ties) and depend only on change content.

The LCM extension is outside core's normalization. The corpus treats it
separately (`equal_under_core_normalization_ignoring_ext`) and records which
relation each alternate artifact has to the primary one, so the difference is
declared rather than hidden.

## What this means for corpus entries

- The primary artifact of a network capture is the data-lake copy: it is what
  Galexie-based pipelines read, and it carries the fuller extension.
- Alternate copies of the same ledger are kept when they exist and are checked
  by `ledgerwitness verify` (`alternate_equivalence`).
- Pointer assertions into a specific artifact may index into change lists,
  because the artifact bytes are fixed. Cross-SDK probes that read changes are
  order-insensitive (see `probes.md`).
- Indexers must not depend on the order of changes within an operation beyond
  the state-then-updated pairing.

## Relation to `--check-lcm` goldens

`--check-lcm` checks LedgerCloseMeta that stellar-core's unit tests produce
(`test-lcm-current/`, `test-lcm-next/`), captured under the test harness's
network configuration with a fixed RNG seed, at the latest protocol only. It
does not take arbitrary files, and network captures cannot be checked against
it directly: they are different ledgers. What the corpus can do is:

1. use the same equivalence relation core uses, as above; and
2. import core's own golden files as a separate source (`source_kind:
   core_test_golden`), verifying them with the same decoder and recording the
   core commit and the test-harness passphrase they were produced under, which
   is not a public network passphrase.

Item 2 is planned and not implemented yet.
