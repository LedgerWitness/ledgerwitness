# Evidence: four SDKs on 5,758 mainnet transactions

`make sweep` puts every transaction of a set of public data-lake ledgers
through the Rust, Go, JavaScript and Python runners and compares them with each other
(`generators/datalake-sweep/`). This run covered 20 mainnet ledgers from
protocol 20 to 29.

## Result

| | |
|---|---|
| Ledgers | 20, from 50500000 (protocol 20) to 65566590 (protocol 29) |
| Transactions | 5,758 (distinct hashes), 13,850 operations of 19 types |
| Runners | stellar-xdr 30.0.0, go-stellar-sdk v0.9.0, @stellar/stellar-sdk 17.4.0, py-stellar-base 17.0.1 |
| Divergences between runners | **0** |
| Decode failures, re-encoding differences, API errors | **0** |

Every transaction decoded in all four SDKs, both as part of its ledger and
as a standalone envelope, re-encoded to the same bytes, and produced the same
value for each of the 14 probes (`docs/spec/probes.md`). Summary:
`compare.json`.

## What the sweep exercised

| Shape | Transactions |
|---|---|
| Failed transactions | 1,877 |
| Fee-bump envelopes | 1,510 |
| Soroban `InvokeHostFunction` | 1,369 |
| Contract events recorded in meta | 968 |
| Operation sources set | 3,394 |
| Accounts with signers in the meta | 76 |
| Trustlines created | 31 |
| Muxed IDs | 5 |
| Pool-share `ChangeTrust` | 0 |

Operation types: PAYMENT 3,750; MANAGE_SELL_OFFER 2,833; MANAGE_BUY_OFFER
2,334; INVOKE_HOST_FUNCTION 1,369; PATH_PAYMENT_STRICT_RECEIVE 1,295;
CREATE_CLAIMABLE_BALANCE 797; PATH_PAYMENT_STRICT_SEND 752;
CLAIM_CLAIMABLE_BALANCE 520; CHANGE_TRUST 67; SET_OPTIONS 50; BUMP_SEQUENCE
19; CREATE_ACCOUNT 17; ACCOUNT_MERGE 9; BEGIN and END_SPONSORING_FUTURE_RESERVES
9 each; ALLOW_TRUST 9; LIQUIDITY_POOL_DEPOSIT 5; CLAWBACK 4;
SET_TRUST_LINE_FLAGS 2.

## Java: two hashes differ from the network

A fifth runner, java-stellar-sdk 6.0.0 (the current release), was added after
the four-SDK run and compared over the same set (`compare-5.json`). It
decodes and re-encodes every ledger and envelope and matches every meta
probe. On 2 of the 5,758 transactions it disagrees with the other four SDKs and
with the network on `tx_hash`: `b4a6d6a2…` and `f38b082e…`, both in ledger
55000000. Both transactions use `PRECOND_V2` with every field at its default
except the time bounds, and Java re-encodes them as `PRECOND_TIME` before
hashing. This is java-stellar-sdk#844, already reported upstream (see
`docs/findings/README.md`, OBS-004). The comparison found it without any
expected value.

## The ledgers are mainnet ledgers

Each ledger was checked with `ledgerwitness check-ledger` against a ledger hash
read from SDF's public history archive (`archive-hash.mjs`), independent of
the data lake (`ledger-checks.txt`):

- the header hash matches the archive on all 20; Horizon still serves 11 of
  them and agrees on all 11 (it returns HTTP 410 for the older 9);
- transaction-set and result-set commitments and the network binding hold on
  all 20;
- 19 headers carry a valid signature from a pinned mainnet validator, from 13
  different validators. Ledger 52000000 carries a valid signature from a key
  that no current stellar.toml declares (its account's home domain is
  whalestack.com), so it is not attested; see architecture §18.

## `to_muxed_id` is usually not a number

CAP-67 transfer events carry a `to_muxed_id` field. For a destination that is
not muxed, stellar-core fills it from the transaction's memo
(`EventManager.cpp`, see `docs/guide/indexer-hardening.md`). Counted over the
transfer events of six of the sweep ledgers:

| Ledger | memo text (`string`) | memo hash (`bytes`) | muxed ID (`u64`) |
|---|---|---|---|
| 64000000 | 97 | 1 | 0 |
| 64500000 | 11 | 0 | 0 |
| 65000000 | 503 | 0 | 0 |
| 65300000 | 113 | 0 | 0 |
| 65566583 | 19 | 0 | 1 |
| 65566590 | 67 | 1 | 1 |
| **Total** | **810** | **2** | **2** |

All 812 non-numeric values come from classic operations. A reader that
expects a 64-bit number in this field fails on almost every event that has
it. wallet-backend, go-stellar-sdk's token-transfer processor and stellar-etl
accept text and bytes; Horizon parses the field only for Soroban
transactions, which cannot carry a memo from protocol 25.

## The comparison can fail

A planted wrong value in one runner's report, and one removed result, are
both reported (`divergence` naming the disagreeing SDK, and `missing`), and
the comparison exits 1.

## Limits

- **Agreement is not correctness.** The sweep has no expected values: the
  SDKs can agree and all be wrong. Correctness is what the curated corpus
  entries check against independent values.
- **Shapes it barely reaches.** Muxed IDs occur in 5 transactions and
  pool-share trustlines in none; history rarely contains the rare shapes,
  which is why the corpus also captures and generates them.
- **Not covered:** ledgers before protocol 20 (LedgerCloseMeta v0, which the
  tools do not read yet) and older SDK releases.

## Reproduce

```sh
npm ci --prefix runners/javascript
make sweep SWEEP_LEDGERS="50500000 50500001 52000000 52000001 54000000 55000000 56500000 58000000 58000001 59500000 60500000 61500000 62500000 63500000 64000000 64500000 65000000 65300000 65566583 65566590"
node generators/datalake-sweep/archive-hash.mjs 65566583   # a ledger hash from the history archive
```

The runner reports (about 8 MB each) are not kept; `make sweep` regenerates
them in `results/sweep/`.
