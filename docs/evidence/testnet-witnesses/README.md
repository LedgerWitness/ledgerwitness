# Evidence: testnet witnesses through eight SDK releases

Crafted cases are only findings once the network accepts them.
`reproductions/sdk-findings/submit-witnesses.py` submits each shape from
fresh friendbot accounts. `witnesses.json` keeps every envelope as Horizon
returns it, with its network hash. Each envelope was then parsed with the
SDK's own transaction model: the SDK must not refuse it, its hash must equal
the network's, and re-encoding must reproduce the original bytes.

"ok" means all three held. Setup transactions (trustlines, issuer flags,
sponsorship) are in `witnesses.json` but not in the table.

| Shape | Ledger | Result | kmp-stellar-sdk 1.18.0 | java-stellar-sdk 6.0.0 | py-stellar-base 17.0.1 | js-stellar-sdk 17.4.0 | js-stellar-sdk 17.2.0 | stellar-dotnet-sdk 17.0.0 | stellar-php-sdk 1.18.0 | stellar_flutter_sdk 3.12.0 |
|---|---|---|---|---|---|---|---|---|---|---|
| `manage-data-space-name` | 5829124 | success | **refused** (#177) | ok | ok | ok | ok | ok | ok | ok |
| `nested-not-predicate` | 5829126 | success | **refused** (#177) | ok | ok | ok | ok | ok | ok | ok |
| `precond-max-ledger-u32` | 5829128 | success | **refused** (#177) | ok | ok | ok | ok | ok | ok | ok |
| `revoke-pool-share-sponsorship` | 5829133 | success | **refused** (#177) | ok | ok | ok | ok | ok | ok | ok |
| `op-source-equals-tx-source` | 5829435 | success | ok | ok | ok | ok | ok | ok | ok | ok |
| `allow-trust-maintain-liabilities` | 5829438 | success | ok | ok | ok | ok | ok | refused (known: #274) | ok | ok |
| `set-trustline-flags-zero` | 5829439 | success | ok | ok | ok | ok | ok | ok | ok | ok |
| `fee-bump-muxed-fee-source` | 5829440 | success | ok | ok | ok | ok | ok | ok | ok | hash ok; signatures dropped on re-encode (known: #300) |
| `claim-predicate-depth-4` | 5829442 | success | ok | ok | ok | ok | ok | ok | ok | ok |
| `path-payment-5-hops` | 5829444 | failed | ok | ok | ok | ok | ok | ok | ok | ok |
| `bump-sequence-zero` | 5829445 | success | ok | ok | ok | ok | ok | ok | ok | ok |
| `set-options-empty` | 5829446 | success | ok | ok | ok | ok | ok | ok | ok | ok |

## What it shows

- **One new finding:** the four refusals in kmp-stellar-sdk, filed as
  Soneso/kmp-stellar-sdk#177 (FND-007).
- **Two known issues reproduced on chain:** stellar-dotnet-sdk refuses
  AllowTrust (Beans-BV/dotnet-stellar-sdk#274), and stellar_flutter_sdk 3.12.0
  drops fee-bump signatures on re-encode (Soneso/stellar_flutter_sdk#300,
  fixed on master).
- **Eight shapes are handled by every SDK:**
  - an operation source equal to the transaction source;
  - AllowTrust to "maintain liabilities";
  - SetTrustLineFlags with zero flags;
  - a fee bump with a muxed fee source whose ID is above 2^63;
  - 10 claimants with predicates nested 4 deep;
  - a failed 5-hop path payment;
  - BumpSequence to 0;
  - an empty SetOptions.

  This negative result is recorded because it narrows where defects remain.
- **One shape core rejects:** a `PRECOND_V2` with `minSeqNum = -1` came back
  `tx_bad_seq`, so it is not a witness and the SDK that refuses it is right.

## Reproduce

```sh
PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/submit-witnesses.py            # batch 1
PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/submit-witnesses.py --batch 2  # batch 2
```

New runs create new accounts and new transactions. The recorded ones are in
`witnesses.json` (testnet ledgers 5829124 to 5829446). Testnet resets
periodically; Horizon and the public data lake keep them until then.
