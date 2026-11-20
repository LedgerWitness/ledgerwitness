# Findings

Observations from running the corpus against Stellar software and
infrastructure. An observation becomes a finding only after triage confirms a
defect. Confirmed findings are reported privately to the affected maintainers
first (see `SECURITY.md`) and published here once disclosed. None has been
filed yet.

Already-fixed upstream bugs that the corpus reproduces are regression tests,
not findings; they are described in `docs/research/source-audit.md`.

## OBS-001: RPC `getLedgers` and the Galexie data lake serve different bytes for the same ledger

- **Observed with:** testnet ledgers 5727135, 5727136, 5727137, 5727139;
  `ledgerwitness compare-lcm`.
- **What differs:** LedgerCloseMeta extension (v0 from RPC, v1 with
  `soroban_fee_write1_kb` from the data lake); order of ledger-entry changes
  within 28 of 76 operations; `invoke_time_nsecs` diagnostic values. Equal
  under stellar-core's `--check-lcm` normalization once the extension is
  ignored.
- **Triage:** not a defect. Change order and timing diagnostics are treated
  as nondeterministic by core's own normalization. The extension depends on
  the emitting node's configuration: stellar-core writes LCM extension v1 only
  when `EMIT_LEDGER_CLOSE_META_EXT_V1` is enabled, and it is off by default.
  Classic-operation events depend on `EMIT_CLASSIC_EVENTS` in the same way.
- **Indexer impact:** consumers that depend on change order within an
  operation, or on the LCM extension being present, behave differently
  depending on their ledger source. This goes into the hardening guide.
- **Severity:** informational.
- **Next step:** none upstream. Corpus entries record which copy is primary
  and how alternates relate to it (`docs/spec/normalization.md`).

## OBS-002: js-stellar-sdk 17.2.0 cannot decode protocol-30 ledger headers

- **Observed with:** every protocol-30 entry; error
  `scpValue.ext.v: unknown enum value 3` (`STELLAR_VALUE_SIGNED_MS`).
- **Triage:** expected. 17.2.0 predates the stellar-xdr change that ungated
  the arm. Classified as an unsupported protocol, not a defect. Current
  releases decode these ledgers.
- **Severity:** none.

## OBS-003: SDF indexers attribute failed operations differently

- **Observed with:** a manual review while running linter rule STLR003 on
  stellar/stellar-horizon `01ba44a` and stellar/wallet-backend `18db88f4`
  (`docs/evidence/linter-stlr003/`).
- **What differs:** for an operation in a *failed* transaction,
  wallet-backend records only the source and fee-bump source as
  participants (since `9538b3ca`, which calls envelope-named accounts on
  failed transactions "unverified submitter input"). Horizon records every
  account the operation body names (`operationsParticipants` in
  `internal/ingest/processors/operations_processor.go` has no success check),
  so a failed payment appears in the destination's operation history.
- **Triage:** not a defect. Horizon ingests failed transactions and shows them
  only when a client asks with `include_failed=true`, whose default is
  `false`; its 30.0.0 changelog describes `include_failed` history built from
  operation bodies. The two indexers made different, documented choices.
- **Indexer impact:** a client that requests failed operations from Horizon
  can show accounts that a third party named in a failed transaction, at the
  cost of a base fee. This goes into the hardening guide's participants
  section. STLR003 cannot detect the pattern: the function also reads meta.
- **Severity:** informational.
- **Next step:** none upstream.

## OBS-004: java-stellar-sdk 6.0.0 rejects a valid pool trustline and mis-hashes two mainnet transactions

- **Observed with:** the Java runner (`runners/java/`, java-stellar-sdk
  6.0.0, the current release, 2026-10-28) on the corpus and on the
  5,758-transaction mainnet sweep (`docs/evidence/mainnet-sweep/`).
- **What differs:**
  1. On `core.testnet.cap38.lp-trustline-issuer-order.p30`,
     `AbstractTransaction.fromEnvelopeXdr` throws "Assets are not in
     lexicographic order" on a successful testnet transaction.
     `AssetTypeCreditAlphaNum4/12.compareTo` orders same-code assets by
     issuer strkey text, while core orders them by key bytes. It is the same
     bug as js-stellar-sdk#1766.
  2. In the mainnet sweep, Java's `hashHex()` differs from the network hash
     for 2 of 5,758 transactions (ledger 55000000: `b4a6d6a2…`,
     `f38b082e…`). Both use `PRECOND_V2` with every field at its default
     except the time bounds. `TransactionPreconditions.hasV2()` is then false,
     so the SDK re-encodes `PRECOND_TIME`. The other four SDKs agree with the
     network.
- **Triage:** confirmed defects, both already reported upstream, so neither
  is a finding of ours. (1) is fixed on master by java-stellar-sdk#832
  (merged 2026-11-05, not released). Verified by execution: with the two
  asset classes compiled from the fix commit `95bca0b5` on the classpath,
  the runner parses the transaction and derives the pool ID core recorded
  (`f21c7468…0b1e`). (2) is java-stellar-sdk#844 (opened 2026-11-18, open).
  The sweep found it without expected values, only by comparing SDKs.
- **Ruled out while triaging:** `ManageDataOperation` and
  `SetOptionsOperation` hold the data name and home domain as Java strings
  through a lossy UTF-8 conversion, but core's `isStringValid` accepts only
  printable ASCII in both fields, so valid ledgers cannot trigger it.
- **Severity:** (1) medium for Java consumers: a valid transaction cannot
  be loaded, and a trustline to such a pool cannot be built. (2) medium: a
  wallet that decodes and re-signs such an envelope signs a different
  transaction, and an indexer that hashes with the SDK records a hash the
  network does not know.
- **Next step:** none upstream. The Java column stays in the matrix as a
  reported, non-gating run until a release includes #832.
