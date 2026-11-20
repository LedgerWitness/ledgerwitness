# Findings

Observations from running the corpus against Stellar software and
infrastructure. An observation becomes a finding only after triage confirms a
defect, reproduces it by execution, and finds no earlier report upstream.
Security-relevant findings are reported privately to the maintainers first
(see `SECURITY.md`). Functional defects are filed as public issues and listed
here once filed. Where the fix is small, a fix pull request follows, with a
test that fails before it: go-stellar-sdk#6063 (FND-003) and
kmp-stellar-sdk#178 (FND-006).

| ID | Component | Defect | Evidence | Upstream |
|---|---|---|---|---|
| FND-001 | stellar-dotnet-sdk 17.0.0 | `Transaction.Hash` is wrong for a `PRECOND_V2` transaction whose fields are at their defaults | 2 mainnet transactions, ledger 55000000 | [Beans-BV/dotnet-stellar-sdk#281](https://github.com/Beans-BV/dotnet-stellar-sdk/issues/281) |
| FND-002 | stellar-php-sdk 1.18.0 | A SAC deployed with `CREATE_CONTRACT_V2` is re-encoded as `CREATE_CONTRACT`, so `hash()` is wrong | testnet transaction `11446413…`, ledger 5822970 | [Soneso/stellar-php-sdk#252](https://github.com/Soneso/stellar-php-sdk/issues/252) |
| FND-003 | go-stellar-sdk v0.9.0 `txnbuild` | `RevokeSponsorship` of a pool-share trustline cannot be built, and its asset encodes as native | testnet transaction `25e65885…`, ledger 5829133; core's own tests cover the shape | [stellar/go-stellar-sdk#6062](https://github.com/stellar/go-stellar-sdk/issues/6062); fix PR [#6063](https://github.com/stellar/go-stellar-sdk/pull/6063) |
| FND-004 | stellar_flutter_sdk 3.12.0 | A SAC deployed with `CREATE_CONTRACT_V2` is re-encoded as `CREATE_CONTRACT`, so the hash is wrong | testnet transaction `11446413…`, ledger 5822970 | [Soneso/stellar_flutter_sdk#305](https://github.com/Soneso/stellar_flutter_sdk/issues/305) |
| FND-005 | kmp-stellar-sdk 1.18.0 (JVM) | `hashHex` is wrong for a `PRECOND_V2` transaction whose fields are at their defaults | 2 mainnet transactions, ledger 55000000 (corpus entry `core.pubnet.cap21.precond-v2-default-fields.p22`) | [Soneso/kmp-stellar-sdk#175](https://github.com/Soneso/kmp-stellar-sdk/issues/175) |
| FND-006 | kmp-stellar-sdk 1.18.0 (JVM) | `fromEnvelopeXdr` refuses a successful `ExtendFootprintTTL` with `extendTo = 0` | testnet transaction `21f5b9f4…`, ledger 3619212 (corpus entry `core.testnet.cap46.extend-ttl-extend-to-zero.p27`) | [Soneso/kmp-stellar-sdk#176](https://github.com/Soneso/kmp-stellar-sdk/issues/176); fix PR [#178](https://github.com/Soneso/kmp-stellar-sdk/pull/178) |
| FND-007 | kmp-stellar-sdk 1.18.0 (JVM) | `fromEnvelopeXdr` refuses four kinds of transaction the network applied | 4 testnet transactions, ledgers 5829124 to 5829133 (`reproductions/sdk-findings/witnesses.json`) | [Soneso/kmp-stellar-sdk#177](https://github.com/Soneso/kmp-stellar-sdk/issues/177) |
| FND-008 | stellar-ios-mac-sdk 3.16.0 | `Transaction(envelopeXdr:)` re-encodes a default-valued `PRECOND_V2` as `PRECOND_TIME`, so the hash is wrong | 2 mainnet transactions, ledger 55000000 | [Soneso/stellar-ios-mac-sdk#330](https://github.com/Soneso/stellar-ios-mac-sdk/issues/330) |
| FND-009 | stellar-ios-mac-sdk 3.16.0 | `Transaction(envelopeXdr:)` drops the Soroban data of a fee bump's inner transaction | 1,358 of 1,511 mainnet fee bumps in the sweep; Horizon-checkable example `06bf9c12…`, ledger 65566583 | [Soneso/stellar-ios-mac-sdk#331](https://github.com/Soneso/stellar-ios-mac-sdk/issues/331) |
| FND-010 | stellar-ios-mac-sdk 3.16.0 | A memo text that is not UTF-8 makes the envelope, and the Horizon page containing it, undecodable | testnet transaction `b7fb286b…`, ledger 5830015 | [Soneso/stellar-ios-mac-sdk#332](https://github.com/Soneso/stellar-ios-mac-sdk/issues/332) |

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

## FND-001: stellar-dotnet-sdk hashes default-valued `PRECOND_V2` transactions as `PRECOND_TIME`

- **Found by:** running the 9,351 mainnet sweep envelopes through
  `Transaction.FromEnvelopeXdr(...).Hash(network)` on stellar-dotnet-sdk
  17.0.0 (the current release; `main` at 6eea5fca is identical).
- **Defect:** `TransactionPreconditions.FromXdr` keeps the field values but
  not the decoded arm, and `ToXdr` picks the arm from `HasV2()`, which is
  false when every V2 field has its default value. A `PRECOND_V2` with only
  time bounds is re-encoded as `PRECOND_TIME`, and one with nothing set as
  `PRECOND_NONE`. The hash, the re-encoded envelope and any signature made
  from the parsed transaction then differ from the original.
- **Evidence:** mainnet `b4a6d6a2…3db9` and `f38b082e…aaf7` (ledger 55000000,
  protocol 22). The SDK returns `8299a5d4…209f` for the first. Both
  transactions were applied, and stellar-core accepts any `PRECOND_V2` from
  protocol 19.
- **Duplicates:** none found in the repository's issues and pull requests.
  The same class of bug is java-stellar-sdk#844 and stellar-php-sdk#222.
- **Severity:** medium. Indexers record hashes that do not exist on chain,
  and a co-signer signs a different transaction.
- **Also seen, not filed:** `TimeBounds` refuses `minTime == maxTime`, which
  core accepts. No on-chain example was found, so it is held back as low
  value.

## FND-002: stellar-php-sdk re-encodes a `CREATE_CONTRACT_V2` SAC deploy as `CREATE_CONTRACT`

- **Found by:** crafted host-function cases through
  `AbstractTransaction::fromEnvelopeBase64XdrString(...)->hash()` on
  stellar-php-sdk 1.18.0 (`main` at 843384c5 is identical). Then confirmed on
  chain: a SAC deploy through `CREATE_CONTRACT_V2` (preimage `FROM_ASSET`,
  executable `STELLAR_ASSET`) was submitted to testnet and succeeded
  (`11446413b5a7a95f8bb18e616a5187f19a5a7b146fdef3c5c2c77e5ea87aaadc`,
  ledger 5822970).
- **Defect:** `DeploySACWithAssetHostFunction::fromXdr` accepts both arms and
  keeps only the asset; `toXdr()` always builds `CREATE_CONTRACT`. The SDK
  hashes the on-chain transaction as `1fc58031…8e55`. Constructor arguments
  of a V2 deploy are also dropped.
- **Cross-check:** java-stellar-sdk 6.0.0 and py-stellar-base 17.0.1 return
  the network hash and the original bytes for the same envelope.
- **Duplicates:** none found; #66/#67 concern a different preimage and
  executable pairing.
- **Severity:** medium-low. The shape is valid but uncommon, because most
  tools deploy a SAC with `CREATE_CONTRACT`.
- **Also seen, not filed:** ManageSellOffer and ManageBuyOffer refuse a
  negative offer ID, which core accepted before protocol 15. No on-chain
  example was found.

## FND-003: go-stellar-sdk `txnbuild` cannot revoke the sponsorship of a pool-share trustline

- **Found by:** crafted cases through `txnbuild` on go-stellar-sdk v0.9.0
  (`main` at 4cee0255 has the same files).
- **Defect:** `RevokeSponsorship.Validate` checks the trustline asset with
  `validateStellarAsset`, which requires an issuer, so `NewTransaction`
  always refuses the operation. Separately,
  `LiquidityPoolShareTrustLineAsset.ToXDR` does not set `Type`, so
  `BuildXDR` returns no error for an operation whose asset encodes as native
  and which core rejects as `REVOKE_SPONSORSHIP_MALFORMED`.
- **Core accepts the shape:** `RevokeSponsorshipOpFrame::doCheckValid` and
  the "pool share trustline" section of `RevokeSponsorshipTests.cpp`.
- **Duplicates:** none for the validation half. The encoding half is fixed in
  passing by the open pull request #5992, which the report credits.
- **Severity:** low-medium. A valid operation cannot be built; no funds at
  risk.
- **On-chain witness (added later):** testnet transaction `25e65885…`
  (ledger 5829133, `txSUCCESS`) revokes the sponsorship of a pool-share
  trustline. With v0.9.0 its hash is correct, but the parsed operation fails
  `Validate` and `BuildXDR` turns its asset into native. Posted to the issue
  as a comment.
- **Checked and clean:** `txnbuild` keeps the original envelope, so parsing
  then hashing or re-encoding was exact on all 9,351 mainnet envelopes.

## FND-004: stellar_flutter_sdk re-encodes a `CREATE_CONTRACT_V2` SAC deploy as `CREATE_CONTRACT`

- **Found by:** the same crafted host-function cases as FND-002, run on
  stellar_flutter_sdk 3.12.0 (`master` at 0d7238dd is identical), then
  confirmed on the testnet transaction from FND-002.
- **Defect:** in `lib/src/invoke_host_function_operation.dart`, the
  `CREATE_CONTRACT_V2` branch maps an asset preimage to
  `DeploySACWithAssetHostFunction`, which keeps only the asset. Its `toXdr`
  calls `XdrHostFunction.forDeploySACWithAsset`, which always builds
  `CREATE_CONTRACT`. The SDK hashes testnet transaction `11446413…` as
  `1fc58031…8e55`, the same wrong value as the PHP SDK. Constructor arguments
  are dropped.
- **Cross-check:** java-stellar-sdk 6.0.0, py-stellar-base 17.0.1 and
  stellar-dotnet-sdk 17.0.0 return the network hash and the original bytes.
- **Duplicates:** none found (searched CREATE_CONTRACT_V2, CreateContractV2,
  DeploySAC, constructor args and wrong hash; scanned issues up to #304).
- **Severity:** medium-low. SEP-7 `signAndSubmitTransaction` and any
  parse-sign-resubmit flow would sign a different transaction.
- **Also seen, not filed:** mismatched preimage and executable pairs throw
  `UnimplementedError` (they can only appear as failed transactions), and a
  SetOptions signer weight above 255, valid before protocol 10, is masked to
  8 bits. Neither has an on-chain example yet.

## FND-005: kmp-stellar-sdk hashes default-valued `PRECOND_V2` transactions as `PRECOND_TIME`

- **Found by:** the mainnet sweep and the corpus through
  `AbstractTransaction.fromEnvelopeXdr(...).hashHex()` on kmp-stellar-sdk
  1.18.0 for the JVM (`main` at 9c11ce11 is identical).
- **Defect:** the same pattern as FND-001. `TransactionPreconditions.fromXdr`
  keeps the field values but not the decoded arm, and `toXdr()` picks the
  arm from `hasV2()`. The SDK hashes mainnet `b4a6d6a2…3db9` as
  `8299a5d4…209f` and re-encodes the envelope in 468 bytes instead of 496.
  The inner transaction of a fee bump is affected too.
- **Duplicates:** none in the repository (searched PRECOND_V2, precondition,
  hasV2, wrong hash). The same class is FND-001 and Soneso/stellar-php-sdk#222.
- **Severity:** medium, as FND-001.
- **Other results of the same run:** a muxed transaction source is turned
  into a G address (known, #170). The crafted refusals were then submitted to
  testnet; see FND-007.

## FND-006: kmp-stellar-sdk refuses a successful `ExtendFootprintTTL` with `extendTo = 0`

- **Found by:** a crafted case in the kmp-stellar-sdk audit, then confirmed on
  chain. Testnet transaction
  `21f5b9f42f5166038d3fb0d1bda5d13cc8b1fc1cdffaedb0bd76680c44180dec`
  (ledger 3619212, protocol 27) has `extendTo = 0` and an empty footprint and
  was applied with `txSUCCESS`. It is the transaction reported in
  lightsail-network/java-stellar-sdk#810.
- **Defect:** `ExtendFootprintTTLOperation`'s `init` block requires
  `extendTo > 0` (`Operation.kt:1879` on `main`), and the parser constructs
  the operation through it, so `fromEnvelopeXdr` throws. stellar-core
  rejects only an `extendTo` above the maximum entry TTL.
- **Other SDKs:** py-stellar-base parses it, and java-stellar-sdk fixed the
  same check in #811. The .NET, PHP, Flutter, Swift and Go sources have no
  such check. The corpus entry passes on every runner in the matrix.
- **Duplicates:** none in the repository (searched extendTo,
  ExtendFootprintTTL, extend footprint).
- **Severity:** low-medium. A valid transaction, and any ledger containing
  it, cannot be loaded through the SDK.

## FND-007: kmp-stellar-sdk refuses four kinds of transaction the network applied

- **Found by:** crafted cases in the kmp-stellar-sdk audit, then confirmed on
  chain. `reproductions/sdk-findings/submit-witnesses.py` submitted each shape
  from fresh testnet accounts, and all four were applied with `txSUCCESS`:
  - a ManageData whose name is one space (`b6ffe005…`, ledger 5829124);
  - a claimable balance with `NOT(NOT(unconditional))` (`c2123aae…`, 5829126);
  - `PRECOND_V2` with `ledgerBounds {0, 2^32-1}` (`5fa3a252…`, 5829128);
  - `RevokeSponsorship` of a sponsored pool-share trustline (`25e65885…`,
    5829133).
- **Defect:** the parser constructs each operation through checks stricter
  than core's: `isNotBlank()` on data names, a ban on nested `NOT`,
  `maxLedger` read with `.toInt()` and then required to be `>= 0`, and an
  explicit `UnsupportedOperationException` for pool-share trustline keys.
- **Other SDKs:** java-stellar-sdk 6.0.0, py-stellar-base 17.0.1,
  js-stellar-sdk 17.4.0 and 17.2.0, stellar-dotnet-sdk 17.0.0,
  stellar-php-sdk 1.18.0 and stellar_flutter_sdk 3.12.0 parse all four,
  return the network hash and re-encode the original bytes. The pool-share revoke is the shape go-stellar-sdk `txnbuild` cannot
  build (FND-003), so it is now an on-chain witness for that finding too.
- **Dropped:** a `PRECOND_V2` with `minSeqNum = -1` was rejected by core with
  `tx_bad_seq`, so the SDK's refusal of it is correct.
- **Duplicates:** none in the repository (searched blank, NOT predicate,
  nest, LedgerBounds, maxLedger, pool share, RevokeSponsorship).
- **Severity:** low-medium. Valid transactions, and the ledgers that contain
  them, cannot be loaded through the SDK.

## FND-008 to FND-010: stellar-ios-mac-sdk 3.16.0

All three were found by running the mainnet sweep, the corpus and the
testnet witnesses through the Swift SDK on Linux. That build vendored the
SDK and replaced only platform code off the XDR and hash path: CommonCrypto
was mapped to OpenSSL, and it was checked against 9,249 network hashes. Each
finding was then re-run on its on-chain example. `master` is the release
commit.

- **FND-008, default-valued `PRECOND_V2`:** the same defect as FND-001 and
  FND-005. `TransactionPreconditions` keeps the values but not the decoded
  arm. Mainnet `b4a6d6a2…` hashes as `8299a5d4…`. SEP-7 signing parses
  envelopes this way. Medium.
- **FND-009, fee-bump Soroban data:** `Transaction(envelopeXdr:)` returns the
  inner transaction of a fee bump, but the `sorobanTransactionData` helper
  returns `nil` for fee-bump envelopes, while every neighbouring helper
  returns the inner field. For mainnet fee bump `06bf9c12…` (ledger
  65566583) Horizon reports the inner hash `7e2fdae0…`; the SDK computes
  `540fb164…`. This affects 1,358 of the 1,511 fee bumps in the mainnet sweep.
  Medium-low.
- **FND-010, memo text that is not UTF-8:** the string decoder throws
  `invalidUTF8`, so the envelope cannot be decoded. `TransactionResponse`
  decodes `envelope_xdr` eagerly, so the receiver's whole Horizon history page
  fails as well; this was confirmed on the real page of the testnet receiving
  account. Anyone can trigger it with one payment. The repository has no
  security policy and private reporting is disabled, and the same class is
  public for the Flutter SDK (#297), so it was filed publicly in neutral terms.
  Medium.
- **Known and not re-filed:** fee rounding to a multiple of the operation
  count (12 mainnet transactions; #83, closed in 2020; fix PR #84 never
  merged), and the v0 hash with several operations (#91).
