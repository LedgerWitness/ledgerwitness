# CAP coverage registry

The denominator for "per-CAP coverage" is the set of CAPs that introduced or
changed something an indexer or SDK can observe in ledger data: a new or
changed XDR shape in envelopes, results, ledger entries or meta, or a new
emission rule (for example "this operation creates a trustline without a
contract event"). CAPs that only change consensus, fees or internal limits
without changing emitted shapes do not need an entry; they are listed with the
reason once audited.

This registry is incomplete. It lists the CAPs that the current corpus
covers and the CAPs named in the public sightings. Building the full list from
`stellar-protocol/core/` is planned work; until then no coverage
percentage is claimed.

| CAP | Protocol | Indexer-visible shape | Corpus entries | Generation | Status |
|---|---|---|---|---|---|
| CAP-0015 | 13 | Fee-bump envelopes: outer hash recorded in `tx_processing`, inner hash inside `txFEE_BUMP_INNER_SUCCESS`; inner fee below the minimum (0); the fee charged to the fee source | `core.pubnet.cap15.fee-bump-zero-inner-fee.p29` | mainnet data lake | covered (plain fee source); a muxed fee source not yet |
| CAP-0021 | 19 | `PRECOND_V2` preconditions; an SDK that chooses the arm from the field values re-encodes a default-valued V2 as `PRECOND_TIME` and computes a wrong hash | `core.pubnet.cap21.precond-v2-default-fields.p22` | mainnet data lake | covered (default fields with time bounds) |
| CAP-0027 | 13 | `MuxedAccount` (M… strkey) as transaction source, operation source and payment destination; 64-bit IDs; as the source of a Soroban operation other than InvokeHostFunction | `core.testnet.cap27.muxed-large-ids-payment.p30`, `core.testnet.cap27.soroban-muxed-extend-ttl.p30` | testnet capture | covered (two shapes) |
| CAP-0038 | 18 | Liquidity pools; pool-share trustlines; pool ID derived from byte-ordered asset pair | `core.testnet.cap38.lp-trustline-issuer-order.p30` | testnet capture | covered (one shape) |
| CAP-0040 | 19 | Ed25519 signed-payload signer key (P… strkey) in SetOptions and account signers | `core.testnet.cap40.signed-payload-signer.p28`, `…-padded.p30` | data lake, testnet capture | covered (two shapes) |
| CAP-0046 (CAP-0046-12) | 20 | `ExtendFootprintTTL` extending a contract instance's TTL; core accepts a muxed source on it (it rejects one only on InvokeHostFunction from protocol 25) | `core.testnet.cap27.soroban-muxed-extend-ttl.p30`, `core.testnet.cap46.extend-ttl-extend-to-zero.p27` (`extendTo = 0`, empty footprint) | testnet capture, data lake | two state-archival shapes; the rest of CAP-0046 not yet |
| CAP-0058 | 22 | `HOST_FUNCTION_TYPE_CREATE_CONTRACT_V2` (contract constructors); also accepted for a Stellar Asset Contract deploy (asset preimage, `stellar_asset` executable) | `core.testnet.cap46.sac-deploy-create-contract-v2.p30` | testnet submission | covered (SAC deploy); a Wasm deploy with constructor arguments not yet |
| CAP-0067 | 23 | Unified asset events for classic operations, including `to_muxed_id` in transfer data | muxed entry (transfer event with u64 `to_muxed_id`) | testnet capture | partly covered |
| CAP-0073 | 26 | SAC `trust(address)` creates a trustline with no event; XLM SAC transfer creates an account | `core.testnet.cap73.sac-trust-trustline.p30` | testnet capture | trustline covered; account creation not yet |
| CAP-0088 | 30 | Millisecond close times: `StellarValueType` `STELLAR_VALUE_SIGNED_MS` in the ledger header, and a signed payload that includes the millisecond close time | all six p30 entries | testnet capture | covered as a header shape; the validator check verifies its signature |

## Required shapes not yet in the corpus

| Shape | Planned source | Class |
|---|---|---|
| Vectors at their maximum length | testnet capture where core accepts the maximum; otherwise synthetic with evidence | core_emitted_valid or synthetic_validated |
| Nested Soroban authorization entries | testnet capture of a contract call with sub-invocations | core_emitted_valid |
| Maps whose keys collide after native conversion | testnet capture as InvokeHostFunction argument | core_emitted_valid |
| Muxed source on ExtendFootprintTtl / RestoreFootprint (wallet-backend #753) | testnet capture | core_emitted_valid |
| Account created by a native SAC transfer (wallet-backend #804) | testnet capture | core_emitted_valid |
| Non-UTF-8 memo text (js-stellar-sdk #1823) | testnet capture | core_emitted_valid |
| Offer IDs above 2^53 (js-stellar-sdk #1785) | testnet capture of a ManageSellOffer on an existing large offer ID | core_emitted_valid |
| Fee-bump transaction with a muxed fee source | testnet capture | core_emitted_valid |
| `TransactionV0` envelope, older `TransactionMeta` versions | pubnet history from the data lake | core_emitted_valid |
| Invalid / non-canonical IDs, discriminants, lengths | hand-crafted | sdk_negative_only |
