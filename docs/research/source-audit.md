# Source audit of public sightings

The public issues and pull requests below document ledger-reading failures the
corpus is meant to catch. Each item records what upstream history establishes
and how far this repository reproduces it. Not every historical issue has been
re-audited to the same depth.

Each item records what the upstream code and issue history establish, the
exact commits involved, the layer the defect lives in, and how far this
repository reproduces it. Labels:

- **VERIFIED FROM UPSTREAM SOURCE**: read from the GitHub API, the upstream
  repository or a published package.
- **VERIFIED BY EXECUTION**: run in this repository; the command is given.
- **UNVERIFIED**: not yet checked.
- **BLOCKED**: could not be done in the current environment; the reason is
  given.

Commit SHAs come from `gh api` (merge commits of the pull requests) or from
`git compare` between release tags. None are inferred from dates.

## js-stellar-sdk

### #1766: asset issuers and ScVal map keys ordered like the network

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): pull request, merge commit
  `c103c9b3e7dc9ea1e47abc01b3a07011ce62af88`. `v17.2.0...v17.2.1` contains it,
  so 17.2.0 is the last release before the fix and 17.2.1 the first with it.
- **Layer:** the SDK's ordering helpers (`Asset.compare`, `scvSortedMap`,
  `nativeToScVal`) and everything built on them, including
  `LiquidityPoolAsset` and the transaction parser. Raw XDR decoding is not
  affected.
- **Reproduction** (VERIFIED BY EXECUTION, `node reproductions/js-sdk/run.mjs`):
  on 17.2.0 `Asset.compare` orders two same-code assets by strkey text and
  `new LiquidityPoolAsset` rejects a pair in network byte order; an account
  address key sorts after a contract address key in `scvSortedMap`. On 17.2.1
  and later all three follow network order.
- **On core-emitted data** (VERIFIED BY EXECUTION, conformance runner): with
  17.2.0, `TransactionBuilder.fromXDR` throws "Assets are not in lexicographic
  order" on the real, successful testnet transaction in
  `core.testnet.cap38.lp-trustline-issuer-order.p30`. 17.4.0 parses it and
  derives the pool ID core recorded.
- **Map-key ordering** (VERIFIED BY EXECUTION): with `Uint8Array` keys, as
  the SDK produces when decoding, 17.2.0 sorts bytes `[10]` before `[9]`
  (the PR's own example) and sorts a symbol key before a string key; 17.2.1
  sorts by raw bytes and puts `SCV_STRING` before `SCV_SYMBOL`, as the host
  does. A `Buffer` key hides the bytes bug, because `Buffer.toString()`
  happens to sort in byte order. u32 keys were never affected (they convert
  to numbers); that case is kept as a control, not counted.

### #1785: muxed account and offer IDs that encode a different ID

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): issue closed 2026-10-08; fixed
  by PR #1780, commit `f9863aebfd81`, first released in 17.3.0. Last release
  before the fix: 17.2.1.
- **Layer:** input validation in the builders (`Operation.manageSellOffer`,
  `MuxedAccount` constructor). This is commonly summarised as IDs that "decode to
  a different ID than they encode"; the defect is in how the SDK accepts input
  values (`"0x10"`, `""`, negative values, unsafe JS numbers), not in decoding
  XDR from the network. The corpus muxed entry with IDs 2^53+1 and 2^64-1
  decodes correctly on 17.2.0 (envelope probes match), which confirms this.
- **Reproduction** (VERIFIED BY EXECUTION): on 17.2.1 an unsafe-number offer ID
  encodes as 9007199254740992, `""` encodes as offer 0 (create new offer),
  `"-1"` is accepted, and `MuxedAccount(…, "0x10").id()` returns `"0x10"` while
  its M-address encodes 16. 17.3.0 rejects all four.
- **Fixture class:** these are API inputs, so they live in the reproduction
  script as SDK-negative cases, not in the network corpus.

### #1789: `scValToNative` drops map entries whose keys collide

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): issue closed 2026-10-07; fixed
  by PR #1778, commit `91bd61d2f8ff`, first released in 17.3.0. js-stellar-sdk
  #1898 reports the LTS line 16.4.1 still drops entries.
- **Layer:** native conversion, not XDR decoding.
- **Reproduction** (VERIFIED BY EXECUTION): on 17.2.1 the map
  `{u32 1: 10, symbol "1": 20}` converts to `{ "1": 20 }`; 17.3.0 throws.
- **Corpus:** no core-emitted ScVal map with colliding keys has been captured
  yet. Planned: capture one as an InvokeHostFunction argument on testnet.

### #1823: XDR string arms change non-UTF-8 bytes

UNVERIFIED beyond the issue text: closed 2026-10-20. Not yet reproduced.

### #1906: the builder refuses a muxed source on every Soroban operation

Not an RFP sighting; found upstream while building a corpus shape, and filed
by a third party on 2026-11-14 (open).

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): `TransactionBuilder`
  (`assertValidSoroban`) refuses a muxed transaction or operation source on
  every Soroban transaction, with the comment "Core also rejects a muxed
  operation source and a memo". stellar-core's
  `TransactionFrame::validateSorobanMemo` (protocol 25 onward) returns early
  unless the single operation is `INVOKE_HOST_FUNCTION`, so a muxed source
  on `ExtendFootprintTTL` or `RestoreFootprint` is valid. 17.2.0 has no such
  check.
- **On the network** (VERIFIED BY EXECUTION, `make testnet-smoke`, shape 5):
  an `ExtendFootprintTTL` with a muxed operation source succeeded on testnet
  ledger 5818357 and extended a contract instance's TTL. Horizon reports the
  operation's `source_account_muxed`. The capture tool sets the muxed source
  in the XDR because the builder will not.
- **Corpus:** `core.testnet.cap27.soroban-muxed-extend-ttl.p30`. Parsing is not
  affected: `TransactionBuilder.fromXDR` in 17.2.0 and 17.4.0 reads the
  transaction, and the conformance runners pass it.
- **Sibling SDKs** (VERIFIED FROM UPSTREAM SOURCE): go-stellar-sdk `4cee025`
  (`txnbuild`) and py-stellar-base `7b5fb3b` have no such check, so neither
  refuses this transaction. No sibling issue is needed. Both also build an
  InvokeHostFunction with a muxed source, which core rejects at submission;
  that is a missing client-side check, not a refusal of valid input.

## js-xdr #165

Issue "Writer primitive methods silently coerce invalid inputs instead of
throwing", closed 2026-10-16. UNVERIFIED beyond the title and state.

## Sibling issues already filed elsewhere

(VERIFIED FROM UPSTREAM SOURCE, timeline cross-references)

- StellarCN/py-stellar-base #1250 (open) and #1252 (open): Python equivalents
  of #1789.
- StellarCN/py-stellar-base #1251 and #1253 (closed): Python equivalent of
  #1785 (out-of-range 64-bit IDs encoded modulo 2^64) and its fix.

## wallet-backend

### #729: CAP-40 signed-payload signers halt ingestion

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): PR merged 2026-09-08, merge
  commit `2c873353c1f0f3c9b145e25a7d009023ad104a1c`; first parent
  `2a0a84ea12523b6c92e278c0c84b443041240095`. The PR names testnet ledger
  4437357 as containing such a signer.
- **Mechanism (from the PR):** the signer state change carries the full `P…`
  strkey into `signer_account_id`; `AddressBytea.Value()` rejects any
  non-muxed key that is not 32 bytes; the COPY fails; the persist transaction,
  including the cursor update, rolls back and ingestion retries the ledger
  forever.
- **Corpus:** `core.testnet.cap40.signed-payload-signer.p28` is ledger 4437357
  from the public data lake, with the signer key confirmed against Horizon.
- **Replay:** `reproductions/wallet-backend/run.sh` runs each commit's own
  `indexer.ProcessLedger` on that ledger and converts every state change with
  that commit's own COPY conversion code (VERIFIED BY EXECUTION): at
  `2a0a84ea` the `SIGNER ADD` row for `PBCK2RYY…NMS` fails conversion ("has a
  40-byte key payload; the account_id column stores only 32-byte
  account/contract keys"); at `2c873353` all 17 state-change rows convert.
  Output in `reproductions/wallet-backend/results/`.
  The database step itself is BLOCKED here (no Postgres role, no TimescaleDB);
  the rollback-and-retry behaviour is taken from the PR, not executed.

### #731: SAC-created trustlines produce no trustline state change (CAP-0073)

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): fixed by PR #734, merge commit
  `c152783c054c765c8b51fd26ed313fd1ebc1c50c` (2026-09-17); first parent
  `764a4f5bcbff5abb0949d93da37f769063013ea0`. Follow-up issue #804 reports a
  related gap for accounts created by a native SAC transfer.
- **Corpus:** `core.testnet.cap73.sac-trust-trustline.p30` (captured with the
  tool in `generators/testnet-capture/`). Core created the trustline (`limit = i64::MAX`,
  `flags = 1`) inside an InvokeHostFunction operation that emitted no contract
  event.
- **Replay** (VERIFIED BY EXECUTION, `reproductions/wallet-backend/run-731.sh`):
  each commit's own `indexer.ProcessLedger` on testnet ledger 5727139.
  - With the pinned go-stellar-sdk v0.7.3, both commits fail to decode the
    ledger ("'3' is not a valid StellarValueType enum value"): v0.7.3 predates
    the protocol-30 `STELLAR_VALUE_SIGNED_MS` header arm.
  - With go-stellar-sdk raised to v0.9.0 at both commits (the same change,
    so the only difference is wallet-backend's code): at `764a4f5b` core's
    trustline `GCAK23MG…|LCKC:GDXIEX2A…` has no TRUSTLINE state change among
    the ledger's 39; at `c152783c` it gets a `TRUSTLINE/ADD` whose token is
    the asset contract the transaction called (`CA4EKC4N…`).
  - `check-results-731.mjs` takes the expected trustline from the corpus
    manifest, not from wallet-backend. Output in
    `reproductions/wallet-backend/results/`. No database write is involved.

### #716: AddressBytea truncates muxed and signed-payload addresses

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): closed 2026-09-18 with no
  linked fix commit. The issue names three producers. Producer 3
  (signed-payload signers) was fixed by #729; producer 1 (muxed source on
  Soroban operations) by #753 (merge `bb0b162b072583c7dabffbdee664d1c117e5ba56`,
  2026-09-25). Producer 2 (SEP-41 muxed ScAddress) was fixed by #693
  (merge `6e5a59101158dc2996295450190142366062cd05`, 2026-08-27), which makes
  the SEP-41 event decoder reduce muxed addresses to their base account.
- **Note:** #693 also added the guard that makes `AddressBytea.Value()` reject
  wrong-length payloads instead of truncating them. At `2a0a84ea` (the #729
  pre-fix commit) that guard is present, so the observable pre-fix failure for
  producer 3 is an encode error, not silent truncation. Silent truncation can
  only be observed before `6e5a5910`.
- **Replay, producer 3** (VERIFIED BY EXECUTION,
  `reproductions/wallet-backend/run-716.sh`): each commit's own
  `indexer.ProcessLedger` on testnet ledger 4437357, then the signer column's
  own `Value()` and `Scan()`, as an insert and a read would. All three commits
  run with their pinned go-stellar-sdk v0.7.3.
  - `79ea2acd` (first parent of #693): the write succeeds silently and stores
    33 bytes; the key reads back as `PBCK2RYY…NSBYVM`, which does not decode
    ("invalid signed payload length 32, expected at least 36"). This is the
    truncation #716 describes.
  - `6e5a5910` (#693): the write is refused ("has a 40-byte key payload").
  - `2c873353` (#729): 41 bytes stored; the key reads back unchanged.
  - `check-results-716.mjs` takes the signer from the corpus manifest. No
    database is involved; the column type performs the conversion itself.
- **Producers 1 and 2: not reproduced.** The corpus has no Soroban operation
  with a muxed source and no SEP-41 event carrying a muxed address. Both need
  new captures before they can be replayed.

### #753: Soroban operation source reduced to its base account

- **Upstream** (VERIFIED FROM UPSTREAM SOURCE): merge
  `bb0b162b072583c7dabffbdee664d1c117e5ba56`. The PR states core rejects a
  muxed source on a single InvokeHostFunction from protocol 25, but
  ExtendFootprintTtl and RestoreFootprint accept one at every protocol.
- **Corpus:** not captured yet (needs a muxed-source ExtendFootprintTtl).

## stellar-etl

(VERIFIED FROM UPSTREAM SOURCE, titles and states only)

- #454 "Amounts stored as lossy float64 …": open.
- #470 "[Epic] Protocol 28 - Data Team": closed 2026-09-28.
- #460 "P23+ event/effect gaps from the old SDK pin: CAP-0073 effects, SAC
  muxed destina…": open.

Not yet analysed against the corpus.

## rs-stellar-xdr

(VERIFIED FROM UPSTREAM SOURCE)

- #581 "Limit capacity reserved when decoding VecM": merged 2026-09-10.
- #598 "Enforce VecM max length on serde deserialize": merged 2026-09-29.
- #604 is an [issue](https://github.com/stellar/rs-stellar-xdr/issues/604),
  not a pull request; it is among the public sightings. Its behavior
  and resolution have not been audited here.

The corpus pins stellar-xdr 30.0.0. Its directly checked
[release notes](https://github.com/stellar/rs-stellar-xdr/releases/tag/v30.0.0)
list both #581 and #598.
`ledgerwitness` passes an explicit length limit equal to the artifact size and a depth
limit of 500 on every decode.

## rs-soroban-sdk #2099

"Fix MuxedAddress panic on non-address ScVal": merged 2026-09-29. Not yet
reproduced.

## stellar-core

- #5435 `--check-lcm` (VERIFIED FROM UPSTREAM SOURCE): merged 2026-09-25,
  merge commit `1b9d421f816c2cd4aae77f9e6692193f37c808c2`. Goldens live in
  `test-lcm-current/` and `test-lcm-next/`, are captured at the latest
  protocol only, and are compared after `normalizeMeta` and
  `zeroNonDeterministicDiagnostics`. See `docs/spec/normalization.md`.
- The emitting core build: testnet RPC reports captive core
  `stellar-core 30.0.0 (0e49751aa9ddf892302cca02cea7f3f5d97df9f1)`; Horizon
  testnet reports the same. The data lake does not publish its emitter's
  build.

## Repositories that moved

- `stellar/go` is archived. The Go SDK is now `stellar/go-stellar-sdk`
  (v0.9.0 is the latest release; wallet-backend pins v0.8.0 on `main`).

## Ecosystem infrastructure used

- Public Galexie data lake: `s3://aws-public-blockchain/v1.1/stellar/ledgers/`
  (`pubnet/`, `testnet/<reset date>/`), zstd, one ledger per object, 64,000
  ledgers per partition. Testnet folders are per reset; the current one is
  `2025-12-18/`.
- Testnet RPC `https://soroban-testnet.stellar.org` (retention 120,960
  ledgers at capture time) and Horizon testnet.
- CAP preambles checked for protocol versions: CAP-0027 (13), CAP-0038 (18),
  CAP-0040 (19), CAP-0067 (23), CAP-0073 (26).
