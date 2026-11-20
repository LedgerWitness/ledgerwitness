# Evidence: five SDKs on 3,593 mainnet transactions from before protocol 20

The second sweep set covers the ledgers the first one could not read:
mainnet before protocol 20, stored as LedgerCloseMeta v0 with a legacy
transaction set. The same five runners compare every transaction with each
other (`generators/datalake-sweep/compare.mjs`).

## Result

| | |
|---|---|
| Ledgers | 20, from 18000000 (protocol 9) to 50000000 (protocol 19) |
| Protocols | 9, 10, 11, 12, 13, 15, 17, 18, 19 |
| Transactions | 3,593 (distinct hashes), 9,345 operations of 20 types |
| Runners | stellar-xdr 30.0.0, go-stellar-sdk v0.9.0, @stellar/stellar-sdk 17.4.0, py-stellar-base 17.0.1, java-stellar-sdk 6.0.0 |
| Divergences between runners | **0** |
| Decode failures, re-encoding differences, API errors | **0** |

Summary: `compare.json`.

## What this set adds

| Shape | Transactions |
|---|---|
| `ENVELOPE_TYPE_TX_V0` envelopes (the pre-CAP-15 format) | 336 |
| `PRECOND_V2` | 93 |
| Fee-bump envelopes | 7 |
| Failed transactions | 1,513 |
| Text, hash and ID memos | 127, 126, 121 |

A V0 envelope has no source-account type: the network hashes it as the
equivalent `ENVELOPE_TYPE_TX` transaction. All five SDKs reproduce the
network's hash for all 336, and Java and Python do so after rebuilding the
transaction from their own models. None of the 93 `PRECOND_V2` transactions
uses only default values, so java-stellar-sdk#844 does not occur here.

Operation types: MANAGE_SELL_OFFER 2,982; MANAGE_BUY_OFFER 1,888; PAYMENT
1,672; CREATE_CLAIMABLE_BALANCE 858; PATH_PAYMENT_STRICT_RECEIVE 811;
PATH_PAYMENT_STRICT_SEND 697; CLAIM_CLAIMABLE_BALANCE 180; CHANGE_TRUST 163;
ALLOW_TRUST 22; SET_OPTIONS 16; SET_TRUST_LINE_FLAGS 16; MANAGE_DATA 12;
CREATE_ACCOUNT 11; ACCOUNT_MERGE 3; BUMP_SEQUENCE 3; BEGIN and
END_SPONSORING_FUTURE_RESERVES 3 each; CREATE_PASSIVE_SELL_OFFER 2; CLAWBACK
2; LIQUIDITY_POOL_WITHDRAW 1.

## The ledgers are mainnet ledgers

`ledger-checks.txt` has one line per ledger from `ledgerwitness
check-ledger`, given the ledger hash read from SDF's history archive
(`archive-hash.mjs`):

- the header hash matches the archive on all 20;
- the transaction-set commitment holds on all 20. For a legacy set the
  header commits to stellar-core's own hash, the previous ledger hash
  followed by each envelope, not to the hash of the set's XDR;
- the result-set commitment and the network binding hold on all 20;
- the headers of protocols 9 to 13 (9 ledgers) are unsigned
  (`STELLAR_VALUE_BASIC`), so they cannot be attested. Of the 11 signed
  headers, 4 carry a signature from a pinned mainnet validator. The other 7
  carry a valid signature from a validator that is not in today's pinned
  set. For these ledgers the history-archive match is the independent check.

## Limits

- **Agreement is not correctness**, as in the first set.
- **Sparse in the oldest years.** Ledgers 18000000 to 26000000 hold 13 to 25
  transactions each.
- **No finding.** The SDKs agree on every probe. The value of this set is
  coverage: the probes, the comparison and the ledger checks now run on
  every protocol from 9 to 30.

## Reproduce

```sh
make sweep SWEEP_DIR=results/sweep-legacy SWEEP_LEDGERS="18000000 20000000 22000000 24000000 26000000 28000000 29000000 30000000 32000000 34000000 36000000 38000000 39000000 40000000 42000000 44000000 46000000 48000000 49000000 50000000"
```
