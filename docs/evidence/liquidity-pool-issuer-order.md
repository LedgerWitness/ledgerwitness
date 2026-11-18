# Evidence: a real testnet transaction an SDK release rejects

One case followed from the raw ledger to the SDK results: a liquidity-pool
trustline that the Stellar network accepted and that JavaScript SDK 17.2.0
refuses to parse.

## Summary

- **The transaction.** It creates a trustline to a liquidity pool for two
  assets with the same code. Their issuers sort in one order as `G…` strings
  and in the opposite order as key bytes.
- **The network accepted it.** Stellar requires the byte order, so the
  transaction succeeded on testnet.
- **JS SDK 17.2.0 rejects it.** It ordered issuers by their string form
  (js-stellar-sdk#1766), so it throws "Assets are not in lexicographic order"
  on this valid transaction.
- **Current SDKs agree with the network.** JS 17.4.0, go-stellar-sdk v0.9.0
  and rs-stellar-xdr 30.0.0 all decode it, and all derive the pool ID that
  stellar-core itself recorded in the ledger.
- **It reproduces live.** A fresh capture submitted to testnet showed the same
  split.

## Why it matters

A wallet, indexer or explorer built on the affected SDK release could not
read a valid transaction from the network. The Go SDK had hit and fixed the
same ordering bug earlier; the fix was not carried across. This is the kind of
defect the corpus turns into a regression test for every SDK at once.

## The corpus entry

`corpus/entries/core-emitted/core.testnet.cap38.lp-trustline-issuer-order.p30/`

| Item | Value |
|---|---|
| Network | testnet (`Test SDF Network ; September 2015`) |
| Ledger | 5727137, protocol 30 |
| Transaction | `cdd0923f15308385f32a124bfe6529ee28608b3912c8951752b9c6a6a7cb90df` (3 ChangeTrust operations, successful) |
| CAPs | CAP-0038 (liquidity pools); CAP-0088 (the protocol-30 header form) |
| Issuer A | `GDXIEX2ASJJNSQ454PDOZICQZM6YSWXKH7VHFPJFSSTXY547BN6LD4KB` (key bytes `ee82…`) |
| Issuer B | `GD4U4D4EI5FDRDTFDW32BW2FBQ5KUO5OS7NCYLRRJQPRS3KTYZQY7PHB` (key bytes `f94e…`) |
| Order | by bytes A < B (the order the network requires); as text B < A (`'4'` sorts before `'X'`) |
| Pool ID | `f21c7468fa092bb33a30e176d657c52026c651acd9ad82eee9c1b49118ad0b1e` |
| Upstream issue | [stellar/js-stellar-sdk#1766](https://github.com/stellar/js-stellar-sdk/pull/1766), fixed in 17.2.1 |

## Where the bytes come from

| Evidence | What it shows |
|---|---|
| `ledger.xdr` is extracted from the public Galexie data lake object `…/testnet/2025-12-18/FFA915FF--5696000-5759999/FFA89C5E--5727137.xdr.zst` (object SHA-256 `eff1e78b…cdebe`) | the raw bytes are public and re-downloadable |
| `ledger.rpc.xdr` is the same ledger from testnet RPC `getLedgers` | a second source; the two are equal under stellar-core's own `--check-lcm` comparison, once the LCM extension is ignored |
| The header hash recomputed from the header bytes equals `dca00826…9046`, the hash Horizon reports for ledger 5727137 | the artifact is that ledger, relative to Horizon's record |
| The header's validator signature verifies against `sdftest2`, one of SDF's published testnet validators | the transaction set was signed by a known validator; this needs no external service |
| Every envelope hashes to its processed result under the testnet passphrase | the data belongs to testnet |
| The focus envelope (`focus-envelope.xdr`) is byte-identical to the envelope inside the ledger | SDKs are tested on the real bytes |

**How the expected values were obtained.** The issuers and pool ID come from
what the capture tool submitted (`generators/testnet-capture/provenance/`).
The pool ID is then confirmed by the liquidity-pool entry stellar-core itself
created in the ledger meta. None of them comes from the SDKs being tested.

## Result

From `make conform`. The table shows the SDKs' own outputs for this entry.

| SDK | Decodes the ledger | Parses the transaction | Pool ID it derives | Verdict |
|---|---|---|---|---|
| rs-stellar-xdr 30.0.0 | yes, re-encodes byte-exact | yes | `f21c7468…0b1e` | pass |
| go-stellar-sdk v0.9.0 | yes, re-encodes byte-exact | yes | `f21c7468…0b1e` | pass |
| @stellar/stellar-sdk 17.4.0 | yes, re-encodes byte-exact | yes | `f21c7468…0b1e` | pass |
| @stellar/stellar-sdk 17.2.0 | no: predates the protocol-30 header form | **throws "Assets are not in lexicographic order"** | none | rejects valid input |

Notes on the table:

- 17.2.0's ledger-decoding failure is a separate, expected version gap: it
  was released before protocol 30. The corpus therefore also ships the
  transaction alone (`focus-envelope.xdr`), which 17.2.0 *can* decode. The
  rejection comes from its transaction parser, on bytes it otherwise reads
  correctly.
- The Rust column checks raw XDR decoding and the runner's own pool-ID hash.
  The Go column uses the SDK's `xdr.NewPoolId`. The JS columns go through the
  SDK's public `TransactionBuilder.fromXDR`, the layer applications use and
  where the bug lives.

## Live reproduction on testnet

`make testnet-smoke` submits the shape again with fresh throwaway keys,
checks the new ledger, and runs both JS SDK releases on it
(`generators/testnet-capture/sdk-check.mjs`). The pool ID it expects is the
one stellar-core wrote into the ledger, read with rs-stellar-xdr, not one
computed by either SDK.

Recorded run of 2026-11-18, 23:35 UTC (`docs/evidence/testnet-run/`):

| Item | Value |
|---|---|
| Ledger | 5804259, protocol 30 |
| Transaction | `d93c7ce14e68ca04589174619328c69d0718b4a2fc042b9641dc33f59c5fc2ed` (3 ChangeTrust operations, successful; Horizon agrees) |
| Issuers | `GAGNBPZD…33SY` (key bytes `0cd0…`) and `GA3XEFMZ…MEWU` (key bytes `3772…`): as text `GA3…` sorts first, as bytes `GAG…` does |
| Header hash | matches Horizon (`25bd1fc9…e9c2`) |
| Validator signature | `sdftest1` |
| Pool ID core recorded | `eb62f293197be8203aa0aff8d6fa811bfd3f8adea7acffa88314696f313885b5` |
| JS SDK 17.4.0 | parses the transaction and derives the same pool ID |
| JS SDK 17.2.0 | **throws "Assets are not in lexicographic order"** |

The same run captured the other three shapes. 17.4.0 read every checked
value exactly as core recorded it (signed-payload signer, muxed IDs 2^53+1
and 2^64−1, the asset contract's `trust` call). 17.2.0 read those three
transactions correctly and failed only on the pool trustline. Fresh runs
produce new accounts and new bytes: what reproduces is the shape and each
SDK's behaviour, not byte identity.

The raw ledgers are saved, so the run can be re-checked without a network.

## Reproduce it

Offline, from a clean checkout:

```sh
npm ci --prefix runners/javascript
make verify            # 5 entries, 5 passed
make conform           # the table above, as results/conformance/matrix.md
make testnet-evidence  # re-checks the recorded live run from its saved bytes
```

Live, on testnet (submits four transactions with throwaway keys):

```sh
npm ci --prefix generators/testnet-capture
make testnet-smoke     # "4/4 fresh testnet captures passed check-ledger"
                       # "OK: 8 SDK observations on 4 fresh captures, 0 unexpected"
```

## Limits

- **The header hash is trusted to its recorded source.** The validator
  signature covers the transaction set and the close time only, not the rest
  of the header. The full header is checked against Horizon's hash, so it is
  only as trustworthy as that record.
- **Ledger meta is not covered by any network hash or signature.** It is
  trusted to the emitter, and corroborated here by a second, independent copy
  of the ledger.
- **One SDK defect, already fixed upstream.** This document shows that the
  corpus catches a known bug on real data. It does not claim a new finding.
