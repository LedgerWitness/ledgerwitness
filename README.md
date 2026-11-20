# LedgerWitness

[![ci](https://github.com/LedgerWitness/ledgerwitness/actions/workflows/ci.yml/badge.svg)](https://github.com/LedgerWitness/ledgerwitness/actions/workflows/ci.yml)

Test data and tools for software that reads the Stellar ledger: SDKs,
indexers, wallet backends and data pipelines.

The project builds a shared kit made of three parts:

- real ledgers containing rare transaction shapes, each with the values a
  correct reader must produce;
- tools that check SDKs and indexers against those ledgers;
- guidance for writing ledger readers that do not break on them.

This repository holds the design of the complete kit and a **proof of
concept** of part of it, running on real Stellar testnet data.

## Evidence at a glance

Every result below is reproducible with the command next to it; all but the
live testnet run work offline.

| What | Result | Rerun |
|---|---|---|
| Corpus | 7 real ledgers (6 testnet, 1 mainnet) covering 8 CAPs, each verified offline against its ledger hash, header commitments and a validator signature | `make verify` |
| SDK conformance | Rust, Go, JS 17.4.0 and Python (py-stellar-base 17.0.1) decode, re-encode byte for byte and produce every expected value on all 7, including a mainnet fee-bump; JS 17.2.0 and java-stellar-sdk 6.0.0, the current Java release, reject a valid pool trustline | `make conform` |
| Known SDK bugs | js-stellar-sdk #1766, #1785 and #1789: 40/40 checks match before and after each fix | `make repro-js` |
| Known indexer bugs | wallet-backend #729, #731 and #716 (two of its three paths) reproduced on the commits before and after each fix | `make repro-wallet-backend` |
| Linter | Rule STLR003 flags the 6 envelope-only participant functions in wallet-backend before #753's fix, and runs clean after it and on current Horizon and stellar-etl | `make lint-wallet-backend` |
| Mainnet sweep | 9,351 mainnet transactions from 40 ledgers (protocols 9 to 29, including 336 pre-CAP-15 V0 envelopes): Rust, Go, JS, Python and Java agree on every probe, with no decode, re-encoding or API failure, except that Java 6.0.0 computes the wrong hash for 2 transactions (java-stellar-sdk#844). All 40 ledgers match SDF's history archive | `make sweep` |
| Strkey decoders | 144 SEP-23 strkeys (34 published, 110 derived from its rules) through six decoders (Rust, Go, two JS releases, Python, Java): all follow SEP-23 except one Go case already reported upstream | `make strkey` |
| New SDK findings | Three defects found by running real transactions through SDKs and filed upstream: stellar-dotnet-sdk 17.0.0 and stellar-php-sdk 1.18.0 compute the wrong hash for transactions the network applied (Beans-BV/dotnet-stellar-sdk#281, Soneso/stellar-php-sdk#252), and go-stellar-sdk `txnbuild` cannot build a valid pool-share `RevokeSponsorship` (stellar/go-stellar-sdk#6062) | `reproductions/sdk-findings/` |
| Live testnet | Five rare shapes submitted and checked; core accepts a Soroban transaction that JS 17.4.0's builder refuses to build (js-stellar-sdk#1906) | `make testnet-smoke` |

Status of every component, and the delivery plan for the rest:
[architecture](docs/architecture.md) (at a glance, §21).

## Why

Every Stellar SDK and indexer decodes the same ledger format (XDR), and each
one meets rare shapes for the first time in production. Some examples:

- a signer that is a signed payload (`P…` address);
- a trustline created by a smart-contract call instead of `ChangeTrust`;
- a muxed account (`M…`) whose ID is too large for a JavaScript number;
- a liquidity pool whose two asset issuers sort one way as text and the other
  way as bytes.

Shapes like these have halted ingestion pipelines and broken SDKs. Each fix
lands in one codebase, and the same bug then appears in the next language.
This kit gives every project the same set of real, awkward ledgers to test
against.

## The complete project

| Deliverable | What the complete project delivers | In the proof of concept |
|---|---|---|
| **Corpus and generator** | <ul><li>One entry per rare shape, tagged with the CAP and protocol that introduced it, with expected decoded values and a per-CAP coverage registry.</li><li>Only ledgers stellar-core actually emitted: from testnet or mainnet history, or generated offline by a pinned local stellar-core under both network passphrases. Hand-made entries are labelled synthetic.</li><li>The shapes that have already broken SDKs: issuer ordering, muxed accounts, IDs that decode to a different value than they encode, vectors at maximum length, nested authorization, map keys that collide after conversion.</li><li>Agreement with stellar-core's `--check-lcm` goldens.</li></ul> | **Implemented:**<ul><li>7 real entries, 6 from testnet and 1 from mainnet, covering 8 CAPs (0015, 0027, 0038, 0040, 0046, 0067, 0073, 0088), protocols 28 to 30</li><li>live testnet generator and data-lake extraction</li><li>port of core's `--check-lcm` comparison</li></ul>**Planned:** offline local-core generator for both passphrases, more mainnet entries, the remaining shapes, golden import, complete CAP registry |
| **Verifier** | Checks that each entry is intact and came from the network it claims, offline. | **Implemented:** hashes, decode/re-encode, header commitments, network binding, validator signature, alternate-copy equivalence |
| **SDK conformance runner** | <ul><li>Runs every entry through the JavaScript, Go and Rust SDKs in CI.</li><li>Reports where they disagree with the network or with each other.</li><li>Opens a sibling issue in each SDK that shares a defect another SDK fixed.</li><li>Reproduces js-stellar-sdk #1766, #1785 and #1789 on their pre-fix releases.</li></ul> | **Implemented:**<ul><li>3 runners and 14 probes</li><li>comparison matrix bound to the exact input files</li><li>the three JS SDK issues reproduced before and after their fixes</li><li>two JS SDK releases checked live on fresh testnet transactions</li></ul>**Planned:** more shapes and releases, sibling-issue workflow |
| **Replay harness** | <ul><li>Feeds the corpus to a real indexer through an input it already reads (mock RPC `getLedgers` first, then the Galexie data-lake layout), using a documented adapter interface.</li><li>Reports each shape the indexer stores wrongly or drops, with bounded memory.</li><li>Its reference adapter for wallet-backend reproduces #729, #731 and #716 before their fixes and passes after.</li></ul> | **Partial:**<ul><li>comparison of stored results against expected changes</li><li>#729 and #731 reproduced on wallet-backend's own indexer at the pre-fix and fix commits, in process (#731 with go-stellar-sdk raised identically at both)</li><li>#716's signed-payload and muxed Soroban-source producers reproduced through the columns' own storage conversion</li></ul>**Planned:** adapter interface, mock RPC, database-backed replay, #716's SEP-41 muxed-address producer |
| **Go indexer linter** | Flags three mistakes in indexer source:<ul><li>trusting fields stellar-core does not validate;</li><li>recursing over authorization trees or nested ScVals without depth or size caps;</li><li>taking participants from the envelope instead of the meta.</li></ul>Flags seeded violations and runs clean on current wallet-backend. | **Prototype of the third rule (STLR003):** seeded cases in CI; on wallet-backend it flags the 6 envelope-only functions of the pre-#753 code and runs clean on the fix and on current `main` ([evidence](docs/evidence/linter-stlr003/README.md)). **Planned:** the other two rules |
| **Indexer-hardening guide** | <ul><li>Which transaction fields core validates, field by field.</li><li>Recommended caps, participants from meta, muxed and signed-payload handling (SEP-0023).</li><li>A checklist matching the linter rules.</li></ul>Its tables are checked against the corpus manifests and reviewed by a core or platform engineer before publication. | **Draft started** ([docs/guide/indexer-hardening.md](docs/guide/indexer-hardening.md)): 8 field rules, each linked to stellar-core source at a pinned commit and to the corpus entry that exercises it, the participants rule and a checklist. **Planned:** remaining fields, measured limits, CI cross-check, review |
| **Findings** | The corpus run against at least one SDF pipeline (wallet-backend or stellar-etl), each failure triaged (severity, CAP, minimal entry) and reported to its maintainers. | Three observations triaged, none a defect; the linter run on wallet-backend, Horizon and stellar-etl; nothing filed yet |
| **Release and maintenance** | Tests and CI for every tool, a tagged release, and maintenance through the first protocol upgrade after delivery. | **Implemented:** CI runs every offline check on each push and pull request. **Planned:** tagged release, upgrade procedure in practice |

### What the proof of concept covers, and why

The proof of concept takes one working slice through each layer, rather
than finishing any one deliverable:

1. **Real data with provenance.** Real testnet ledgers whose origin is
   checked offline.
2. **SDK conformance.** Three SDKs compared on those ledgers.
3. **Known bugs reproduced.** Upstream bugs reproduced before and after
   their fixes.
4. **Live check.** One command that repeats all of it on testnet.
5. **Linter rule.** One of the three linter rules, checked on wallet-backend
   before and after the fix for that mistake.

The other two linter rules, the guide, database-backed replay and offline
generation are designed in [docs/architecture.md](docs/architecture.md) but
not built.

## See it work on testnet

```sh
npm ci --prefix generators/testnet-capture
npm ci --prefix runners/javascript
make testnet-smoke
```

This command:

1. submits five rare shapes to Stellar testnet, using throwaway accounts
   funded by friendbot;
2. captures each resulting ledger exactly as the network served it;
3. checks every capture against the ledger hash Horizon reports, the validator
   signature in the header, and the submitted transaction;
4. runs JS SDK 17.4.0 and 17.2.0 on every capture, and compares what they read
   with what stellar-core recorded in the ledger and with what was submitted.

The recorded run of 2026-11-18 ([docs/evidence/testnet-run/](docs/evidence/testnet-run/)),
made before the fifth shape was added, ends with:

```
4/4 fresh testnet captures passed check-ledger
OK: 8 SDK observations on 4 fresh captures, 0 unexpected
```

In that run, SDK 17.4.0 read every value as core recorded it. SDK 17.2.0
rejected the valid pool trustline in ledger 5804259 with "Assets are not in
lexicographic order". The raw ledgers are saved, so `make testnet-evidence`
re-checks the run offline; CI does this on every push.

[docs/evidence/liquidity-pool-issuer-order.md](docs/evidence/liquidity-pool-issuer-order.md)
follows that case from the raw ledger to the result.

## Quick start (offline)

Requirements: Rust (the version in `rust-toolchain.toml` installs
automatically), Go 1.26 or later, Node.js 22, Python 3.10 or later, a JDK 17 or later, and
`make`. Installing
dependencies uses the network; the commands themselves run against the local
corpus and do not contact a Stellar network.

```sh
npm ci --prefix runners/javascript
npm ci --prefix reproductions/js-sdk
python3 -m pip install --target runners/python/.deps -r runners/python/requirements.txt

make verify      # check every corpus entry       -> "7 entries, 7 passed, 0 failed"
make test        # unit, tamper and CLI tests
make conform     # run the SDK runners and compare -> prints the table below
make repro-js    # JS SDK bugs before/after fixes -> "40/40 ... match the documented behaviour"
make testnet-evidence  # recorded live testnet run, re-checked offline
make linter      # linter rule STLR003 on its seeded cases
```

`make conform` fails if a current SDK release disagrees with the expected
values. JS 17.2.0 (an older release) and Java 6.0.0 (whose fix is not
released yet) are reported but do not fail the build.

## Results

Recorded with the pinned versions in this repository. A newer build needs a
new run.

| Corpus entry | Rust (stellar-xdr 30.0.0) | Go (go-stellar-sdk v0.9.0) | JS 17.4.0 | Python (py-stellar-base 17.0.1) | JS 17.2.0 | Java (java-stellar-sdk 6.0.0) |
|---|---|---|---|---|---|---|
| Payment between muxed accounts, IDs 2^53+1 and 2^64−1 | pass | pass | pass | pass | ledger unsupported | pass |
| Liquidity-pool trustline, issuers sort differently as text and bytes | pass | pass | pass | pass | **rejects valid input** | **rejects valid input** |
| Signed-payload signer with a padded payload | pass | pass | pass | pass | ledger unsupported | pass |
| Signed-payload signer, the ledger from wallet-backend#729 | pass | pass | pass | pass | pass | pass |
| Trustline created by the asset contract's `trust` function | pass | pass | pass | pass | ledger unsupported | pass |
| `ExtendFootprintTTL` with a muxed operation source (JS 17.4.0's builder refuses to build it) | pass | pass | pass | pass | ledger unsupported | pass |
| Mainnet fee-bump with an inner fee of 0, wrapping a payment whose operation source differs from the transaction source | pass | pass | pass | pass | pass | pass |

What the labels mean:

- **pass**: the SDK decoded the entry, re-encoded it to the identical bytes,
  and produced every expected value.
- **ledger unsupported**: the SDK is too old to read a protocol-30 ledger
  header. The transaction itself still decodes correctly. This is a version
  gap, not a bug.
- **rejects valid input**: the SDK refused data the network accepted.

What the results show:

- **JS SDK 17.2.0 rejects a real transaction.** It is the last release before
  js-stellar-sdk#1766 was fixed. It rejects a successful testnet transaction
  with "Assets are not in lexicographic order"; 17.4.0 handles it.
- **The current Java release has the same bug.** java-stellar-sdk 6.0.0
  (2026-10-28) orders same-code assets by issuer strkey text, so
  `AbstractTransaction.fromEnvelopeXdr` throws on the same transaction. The
  fix (java-stellar-sdk#832) is on master but not yet released. With the
  two asset classes from the fix commit on the classpath, the runner parses
  the transaction and derives the pool ID core recorded.
- **Old JS bugs reproduce exactly.** For js-stellar-sdk #1766, #1785 and
  #1789, all 40 checks show the documented behaviour before and after each
  fix. Each throwing case must produce the documented error message, and the
  run also includes four controls.
- **wallet-backend#729 reproduces in process.** On the testnet ledger named in
  that issue, the pre-fix code cannot convert the `P…` signer for storage.
  The fixed code converts all 17 changes.
  - This was not a database write; see [architecture §11.2](docs/architecture.md).
  - The signer row's transaction ID matches the one Horizon reports
    independently.
- **wallet-backend#731 reproduces in process.** On the real CAP-0073 testnet
  ledger, core creates a trustline through the asset contract's `trust`
  function. Before the fix, wallet-backend records no trustline change for
  it; after the fix, it records `TRUSTLINE/ADD`.
  - Both commits pin a Go SDK too old for protocol-30 ledgers, so the SDK is
    raised identically at both; the original-pin failure is recorded too.
- **wallet-backend#716's silent truncation reproduces.** Through the signer
  column's own storage conversion, the `P…` signer of the #729 ledger is
  stored as 33 bytes without error and reads back as an address that does not
  decode. After the length guard (#693) the write is refused; after #729 the
  full key is stored and read back unchanged.
  - The same happens to a muxed (`M…`) source of a Soroban operation, using the
    corpus `ExtendFootprintTTL` entry: stored as an undecodable value before
    #693, as its base account from #693, and no longer produced from #753.
  - This covers two of #716's three producers; the SEP-41 muxed-address
    producer needs a ledger shape the corpus does not have yet.
- **Core accepts what an SDK refuses to build.** An `ExtendFootprintTTL` with
  a muxed operation source succeeded on testnet (ledger 5818357), although JS
  SDK 17.4.0's builder refuses it; core rejects a muxed source only on
  InvokeHostFunction. This is js-stellar-sdk#1906, already reported upstream.
  The Go and Python SDKs do not share the refusal.
- **The linter catches a real fix.** STLR003 reports the 6 functions that took
  Soroban participants from the envelope in wallet-backend before `9538b3ca`
  (#753), none after it, and none on current Horizon or stellar-etl
  ([evidence](docs/evidence/linter-stlr003/README.md)).
- **Two sources serve the same ledger differently.** The public data lake and
  RPC `getLedgers` return different bytes for the same ledger: an extension
  field, the order of some changes, and timing values. The two copies agree
  under stellar-core's own comparison rule once the extension is ignored
  ([details](docs/spec/normalization.md)).

## How it works

1. **Capture.** Ledgers come from public history (the Galexie data lake on
   AWS), or from transactions submitted to testnet with throwaway keys
   (`generators/testnet-capture/`).
2. **Describe.** Each ledger gets a manifest (`entry.json`) recording where it
   came from and what a correct reader must produce. Expected values come
   from what was submitted, from independent sources such as Horizon, or from
   protocol rules, never from the SDK being tested.
3. **Verify.** `ledgerwitness verify` checks every entry offline (next section).
4. **Compare.** Each SDK runner computes the same 14 named values (*probes*)
   for every entry, such as the transaction hash, addresses, muxed IDs, pool
   IDs, and the signers and trustlines the ledger recorded.
   - Every runner records the SHA-256 of the exact files it read.
   - `ledgerwitness conform` refuses any report produced from other inputs, then
     compares the answers with the expected values and with each other.

## What a passing entry proves

- **The bytes are intact.** File hashes match the manifest, and decoding then
  re-encoding gives the identical bytes.
- **The header matches the recorded ledger hash.** The header hash, recomputed
  from the header, equals the hash recorded from Horizon. The header's
  commitments to the transaction set and the results are recomputed too.
- **It is that network.** Every transaction hashes to its recorded result only
  under the declared network passphrase.
- **A known validator signed the transaction set.** The validator's signature
  over the transaction-set hash and close time is checked offline against
  pinned validator keys: SDF's testnet validators, and 30 mainnet validators
  whose sources are kept in [docs/evidence/mainnet-validators/](docs/evidence/mainnet-validators/README.md).
  Entries that cannot be checked this way are rejected.
- **Copies agree.** Where a second copy of the ledger exists, the two copies
  are equivalent.

What it does not prove:

- **The rest of the header.** The validator signature does not cover the
  ledger sequence or the results; those are only as trustworthy as the
  recorded ledger hash.
- **Changes and events.** The per-transaction changes and events inside a
  ledger are not covered by any network hash or signature. They are trusted
  to the source, and cross-checked where a second copy exists.
- **Consensus.** The check is one validator's signature, not a consensus
  proof.

Full trust model: [architecture §7](docs/architecture.md).

## Using the corpus in your own project

The corpus is plain files: raw XDR plus JSON. You do not need any tool from
this repository to use it. [corpus/README.md](corpus/README.md) explains the
format and the minimal check every consumer should run.

## More commands

```sh
# Check a raw captured ledger against an independently obtained ledger hash
cargo run -q -p ledgerwitness -- check-ledger ledger.xdr --tx-hash <tx-hash> --ledger-hash <hash-from-horizon>

# Show a decoded entry as JSON (optionally one field)
cargo run -q -p ledgerwitness -- inspect LedgerCloseMeta \
  corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/ledger.xdr \
  --pointer /v2/tx_processing/0/tx_apply_processing/v4/operations/0/changes/1

# Add a ledger from the public data lake
cargo run -q -p ledgerwitness -- galexie-extract <object.xdr.zst> --ledger <seq> --out ledger.xdr
cargo run -q -p ledgerwitness -- locate ledger.xdr <tx-hash>
cargo run -q -p ledgerwitness -- extract-envelope ledger.xdr <tx-hash> --out focus-envelope.xdr

# Compare an indexer's observed changes with an expected-result file
cargo run -q -p ledgerwitness -- replay-check --entry <entry-dir>/entry.json --oracle <oracle.json> --observed <observation.json>

# Replay wallet-backend #729, #731 and #716 (needs a clone of stellar/wallet-backend)
make repro-wallet-backend WALLET_BACKEND=../wallet-backend
```

How `ledgerwitness conform` exits:

- with `--require-pass <library>`, only the named libraries decide the exit
  code;
- with `--report-only`, it always exits 0;
- with neither, any non-pass result makes it exit 1.

`cargo run -q -p ledgerwitness -- --help` lists every command.

## Repository layout

```
corpus/          the corpus: schema, entries, expected replay results
crates/ledgerwitness/      the ledgerwitness command-line tool and library (Rust)
crates/replay/   replay comparison library (Rust)
runners/         SDK runners: rust/, go/, javascript/, python/, java/
linter/go/       indexer linter prototype (rule STLR003) and its seeded cases
generators/      testnet capture tool, live smoke test, capture provenance
reproductions/   known bugs, before and after their fixes
docs/            architecture, specifications, evidence, research, findings
```

## Documentation

| Document | Contents |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Full technical design: trust model, implemented and planned components, requirements traceability |
| [docs/guide/indexer-hardening.md](docs/guide/indexer-hardening.md) | Draft guide for indexer authors: what stellar-core checks, field by field, with source links |
| [docs/evidence/liquidity-pool-issuer-order.md](docs/evidence/liquidity-pool-issuer-order.md) | One case from raw ledger to SDK results, with a live testnet reproduction |
| [docs/evidence/testnet-run/](docs/evidence/testnet-run/README.md) | The recorded live testnet run: raw ledgers, provenance, logs and SDK results |
| [docs/evidence/linter-stlr003/](docs/evidence/linter-stlr003/README.md) | Linter rule STLR003 on wallet-backend before and after its participants fix |
| [docs/evidence/mainnet-sweep/](docs/evidence/mainnet-sweep/README.md) | Five SDKs on 9,351 mainnet transactions (protocols 9 to 29, in two sets), and how the ledgers were verified |
| [docs/evidence/mainnet-validators/](docs/evidence/mainnet-validators/README.md) | Sources of the pinned mainnet validator keys |
| [docs/evidence/strkey-sep23/](docs/evidence/strkey-sep23/README.md) | Six strkey decoders against SEP-23's published and rule-derived cases |
| [corpus/README.md](corpus/README.md) | Corpus format for consumers |
| [docs/spec/probes.md](docs/spec/probes.md) | The 14 values every SDK runner computes, and the report format |
| [docs/spec/normalization.md](docs/spec/normalization.md) | When two copies of a ledger count as the same |
| [docs/spec/replay.md](docs/spec/replay.md) | Replay comparison format |
| [docs/research/source-audit.md](docs/research/source-audit.md) | The upstream bugs behind each entry |
| [docs/research/coverage-registry.md](docs/research/coverage-registry.md) | Which CAPs and shapes the corpus covers |
| [docs/findings/README.md](docs/findings/README.md) | Findings filed upstream, and observations from running the corpus against Stellar software |

## Network use and privacy

Verification, tests and comparisons run offline. Only the capture tool and
`make testnet-smoke` contact a Stellar network, and only testnet. The same goes
for the one-time downloads that produced the corpus. The endpoints are:

- the public data lake (`aws-public-blockchain.s3.us-east-2.amazonaws.com`);
- testnet RPC (`soroban-testnet.stellar.org`);
- Horizon testnet;
- testnet friendbot.

The capture tool never writes keys to disk. The corpus holds only public
testnet data. Nothing here collects usage data.

Nothing needs to be hosted. Anyone can mirror the corpus: every file is
pinned by SHA-256, and verification checks each ledger against network
commitments and validator keys offline, without trusting the server it came
from. See [architecture §22](docs/architecture.md).

The Rust tools refuse oversized inputs (limits in `crates/ledgerwitness/src/input.rs`).
The Go and JavaScript runners do not cap their inputs yet.

## Contributing, security, license

- [CONTRIBUTING.md](CONTRIBUTING.md): setup, checks, and how to add a corpus entry.
- [SECURITY.md](SECURITY.md): report vulnerabilities privately, including
  defects this kit finds in Stellar software.
- [CHANGELOG.md](CHANGELOG.md): notable changes.
- Roadmap: [architecture §21](docs/architecture.md), the delivery plan.
- License: Apache-2.0 (see `LICENSE` and `NOTICE`).
