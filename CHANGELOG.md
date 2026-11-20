# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). The manifest schema and the probe
spec are versioned separately (`schema_version`, `probe_spec`).

## [Unreleased]

### Added
- Corpus manifest schema 0.1.0 and five core-emitted testnet entries
  (CAP-0027, CAP-0038, CAP-0040, CAP-0067, CAP-0073; protocols 28 and 30).
- `ledgerwitness`: verify, inspect, locate, galexie-extract, extract-envelope,
  compare-lcm, conform, replay-check.
- Offline validator attestation: the header's `StellarValue` signature is
  verified against pinned testnet validator keys.
- Port of stellar-core's LedgerCloseMeta normalization used by `--check-lcm`.
- Conformance runners for Rust, Go and JavaScript; divergence matrix.
- Replay comparison format 0.1.0 and three curated replay oracles.
- Reproductions of js-stellar-sdk #1766, #1785, #1789 and wallet-backend #729.
- Testnet capture generator.

### Changed
- The wallet-backend #729 checker requires the specific payload-length
  rejection on the pre-fix side, not any error.
- README rewritten for first-time readers: quick start with expected output,
  result labels explained, plain-language summary of what verification proves.
- Probe spec 0.2.0: runner reports record SHA-256 digests of the manifest,
  ledger and focus envelope they read; `ledgerwitness conform` refuses reports produced
  from other inputs. 0.1.0 reports are no longer accepted.
- `ledgerwitness conform` exits 1 on any non-pass cell unless `--require-pass` or
  `--report-only` is given; new `--schema` option.
- Core-emitted entries fail verification when they cannot be attested by a
  pinned validator or contain no transactions.
- Manifests: CAP-0088 tagged on protocol-30 entries; p28 payload description
  corrected; regression links narrowed; emitter-dependent events marked.

### Added
- `ledgerwitness check-ledger` checks a raw captured ledger without a manifest.
- `make testnet-smoke`: live testnet capture checked against Horizon and the
  validator signature, with a recorded run in `docs/evidence/testnet-run/`.
- `docs/evidence/liquidity-pool-issuer-order.md`: one case from raw ledger to
  SDK results.

- `generators/testnet-capture/sdk-check.mjs`: runs JS SDK 17.4.0 and
  17.2.0 over fresh testnet captures and compares what they read with what
  core recorded and with the submitted values; `make testnet-smoke` runs it.
- `linter/go/`: prototype of linter rule STLR003 (participants from the
  envelope with no meta input) on go/analysis, with seeded cases run in CI.
  `make lint-wallet-backend` runs it on wallet-backend: 6 findings before the
  #753 Soroban-participants fix, none at the fix or on `main`.
- wallet-backend #731 reproduction (`run-731.sh`): on the CAP-0073 testnet
  ledger, no trustline state change before the #734 fix and `TRUSTLINE/ADD`
  after it, with go-stellar-sdk raised identically at both commits.
- wallet-backend #716 reproduction for its signed-payload producer
  (`run-716.sh`): silent truncation to an undecodable address before #693, a
  refused write at #693, an exact round trip from #729.
- Corpus entry `core.testnet.cap27.soroban-muxed-extend-ttl.p30`:
  ExtendFootprintTTL with a muxed operation source on testnet ledger
  5818357, which core accepts and js-stellar-sdk 17.4.0's builder refuses
  (js-stellar-sdk#1906). Rust, Go and JS 17.4.0 pass it.
- wallet-backend #716 reproduction for its muxed Soroban operation source
  (`run-716-soroban.sh`) on the new ExtendFootprintTTL entry: undecodable
  before #693, stored as the base account from #693, no longer produced from
  #753.
- Mainnet validator pins: 30 keys from SDF's quickstart pubnet configuration
  and the stellar.toml of Range, MoneyGram and YLDS, with copies of every
  source; 40 sampled mainnet ledgers pass `check-ledger`.
- First mainnet corpus entry, `core.pubnet.cap15.fee-bump-zero-inner-fee.p29`:
  a fee-bump with an inner fee of 0 on ledger 65566583, attested by a pinned
  mainnet validator. Rust, Go and both JS releases pass it, which also
  exercises the runners' fee-bump hashing and inner-source handling on real
  data for the first time.
- Mainnet sweep: 5,758 transactions from 20 mainnet ledgers (protocols 20
  to 29) through Rust, Go and JS with no difference; ledgers checked against
  SDF's history archive (`archive-hash.mjs`) and validator signatures.
- Python runner (py-stellar-base 17.0.1): passes all seven corpus entries
  and agrees with Rust, Go and JS on all 5,758 sweep transactions.
- `make strkey`: 144 SEP-23 strkeys through Rust, Go, two JS releases and
  Python; one Go result is a known upstream issue (go-stellar-sdk#6042).
- Java runner (java-stellar-sdk 6.0.0, `runners/java/`, jars pinned by
  SHA-256): a reported, non-gating column. It rejects the CAP-38 issuer-order
  entry (java-stellar-sdk#832, fixed on master and not yet released). In the
  mainnet sweep it hashes 2 of 5,758 transactions differently from the
  network (java-stellar-sdk#844).
- Corpus entries `core.pubnet.cap21.precond-v2-default-fields.p22` (mainnet
  ledger 55000000) and `core.testnet.cap46.sac-deploy-create-contract-v2.p30`
  (testnet ledger 5822970): the on-chain witnesses of the hash findings. All
  current SDK runners pass both; java-stellar-sdk 6.0.0 hashes the first
  wrongly (#844).
- Corpus entry `core.testnet.cap46.extend-ttl-extend-to-zero.p27` (testnet
  ledger 3619212): `ExtendFootprintTTL` with `extendTo = 0`, the on-chain
  witness of Soneso/kmp-stellar-sdk#176.
- Testnet witnesses (`submit-witnesses.py`, `witnesses.json`): 21 shapes
  applied on testnet and parsed by nine SDK releases
  (`docs/evidence/testnet-witnesses/`).
- Fix pull requests for two findings: stellar/go-stellar-sdk#6063
  (pool-share `RevokeSponsorship`) and Soneso/kmp-stellar-sdk#178
  (`extendTo = 0`), each with a test that fails before the fix.
- First ten findings filed upstream, each with a reproduction in
  `reproductions/sdk-findings/`: Beans-BV/dotnet-stellar-sdk#281 (wrong
  hash for default-valued `PRECOND_V2`, 2 mainnet transactions),
  Soneso/stellar-php-sdk#252 (wrong hash for a `CREATE_CONTRACT_V2` SAC
  deploy, confirmed with a testnet transaction), and
  stellar/go-stellar-sdk#6062 (`txnbuild` cannot revoke the sponsorship of a
  pool-share trustline), and Soneso/stellar_flutter_sdk#305 (the same
  `CREATE_CONTRACT_V2` defect as the PHP SDK), and Soneso/kmp-stellar-sdk#175
  (the default-valued `PRECOND_V2` defect, on the corpus entry), and
  Soneso/kmp-stellar-sdk#176 (refuses a successful `ExtendFootprintTTL` with
  `extendTo = 0`), and Soneso/kmp-stellar-sdk#177 (refuses four more kinds of
  applied transaction, witnessed with `submit-witnesses.py`), and
  Soneso/stellar-ios-mac-sdk#330, #331 and #332 (default `PRECOND_V2`, fee-bump
  Soroban data, memo text that is not UTF-8).
- LedgerCloseMeta v0 (legacy transaction sets, before protocol 20) in the
  CLI and all runners. A second mainnet sweep of 3,593 transactions from 20
  ledgers (protocols 9 to 19, 336 V0 envelopes) found no divergence across
  the five SDKs. All 20 ledgers match SDF's history archive.
- `make testnet-evidence`: offline re-check of the recorded live run, also
  run in CI. The recorded run now keeps the raw ledgers (testnet ledgers
  5804257–5804261).

### Fixed
- Runners report failed entries under their ID; Rust catches panics per entry;
  Go reports muxed-destination decode errors; JS maps REVOKE_SPONSORSHIP.
- Runner library labels are derived from the build.
- Absent-only replay oracles are inconclusive instead of passing.
- wallet-backend replay script no longer certifies stale reports.

