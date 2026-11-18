# Recorded live testnet run

Output of `make testnet-smoke`, run on 2026-11-18 between 23:34:57 and
23:36:03 UTC against Stellar testnet (`Test SDF Network ; September 2015`).
The run submitted four transactions from fresh friendbot-funded accounts,
captured each ledger from testnet RPC `getLedgers`, and checked it.

| Shape | CAP | Ledger | Transaction |
|---|---|---|---|
| Signed-payload signer, 29-byte payload | CAP-0040 | 5804257 | `9daafa395f5981d107d6076b9d30d1ddd91c2c135cd874070b1c6a29ee7695df` |
| Payment between muxed accounts, IDs 2^53+1 and 2^64−1 | CAP-0027 | 5804258 | `cefa0af8e191fce2c3ab80860dbd6d61516005166861f29e526e4d3c56dc6d3e` |
| Pool trustline, issuers sort differently as text and bytes | CAP-0038 | 5804259 | `d93c7ce14e68ca04589174619328c69d0718b4a2fc042b9641dc33f59c5fc2ed` |
| Trustline created by the asset contract's `trust` function | CAP-0073 | 5804261 | `06302f6fc1b574a870fcf4a429e37b790c1907cbb7757e873cf1581f5b09418c` |

All four ledgers are protocol 30. Horizon testnet reported all four
transactions as successful in these ledgers at 23:36:45 UTC.

## Files

| File | Contents |
|---|---|
| `<shape>.LedgerCloseMeta.xdr` | the ledger exactly as `getLedgers` returned it (base64-decoded, not re-encoded) |
| `<shape>.provenance.json` | RPC endpoint, ledger, ledger hash and close time reported by RPC, transaction hash and the values the capture tool submitted |
| `smoke.log` | full output of the run: capture, `check-ledger` against the ledger hash fetched from Horizon, and the SDK check |
| `sdk-check.json` | the SDK check as JSON |

No keys were written: the accounts were throwaway and lived only in memory.

## Results

- **4/4 ledgers passed `check-ledger`:**
  - the header hash equals the hash Horizon reports for that ledger;
  - the transaction-set and result-set hashes recompute from the ledger;
  - every transaction hashes to its result under the testnet passphrase;
  - the header carries a valid signature by an SDF testnet validator
    (`sdftest1` or `sdftest2`);
  - the submitted transaction is present.
- **8/8 SDK observations match the documented behaviour**
  (`generators/testnet-capture/sdk-check.mjs`):
  - `@stellar/stellar-sdk` 17.4.0 decodes all four ledgers and reads every
    checked value exactly as core recorded it;
  - 17.2.0 cannot decode protocol-30 headers (an expected version gap). It
    reads three of the four transactions correctly and **rejects the valid
    pool trustline with "Assets are not in lexicographic order"**
    (js-stellar-sdk#1766).

## Re-check offline

```sh
npm ci --prefix runners/javascript
make testnet-evidence
```

This re-runs the ledger checks and the SDK check on the saved bytes, with no
network access. Offline, the ledger hash comes from the RPC record in the
provenance file. The match against Horizon is recorded in `smoke.log` from
capture time. The validator signature is checked offline against pinned keys
and does not depend on either service.

A new `make testnet-smoke` run produces new accounts, ledgers and bytes.
What reproduces is the shape and each SDK's behaviour, not the bytes.
