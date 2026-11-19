# Mainnet validator keys

`ledgerwitness` checks the validator signature in every ledger header against
keys pinned per network (`crates/ledgerwitness/src/attest.rs`). This folder
keeps the sources of the 30 mainnet keys, so the pinned set can be audited.

| Source | Keys | File | SHA-256 |
|---|---|---|---|
| SDF's quickstart image, `pubnet/core/etc/stellar-core.cfg` at stellar/quickstart@35a3f46e9957 (2026-11-12) | 21 (LOBSTR, SatoshiPay, Blockdaemon, PublicNode, OBSRVR, Franklin Templeton, SDF) | `quickstart-pubnet-stellar-core.cfg` | `336a2d68…3c90f3` |
| https://stellar.validator.range.org/.well-known/stellar.toml | 3 (Range) | `range.stellar.toml` | `246b1965…08634b` |
| https://www.moneygram.com/.well-known/stellar.toml | 3 (MoneyGram) | `moneygram.stellar.toml` | `b218f23e…cfb2726` |
| https://www.ylds.com/.well-known/stellar.toml | 3 (YLDS) | `ylds.stellar.toml` | `b78cf5e4…3c5c81` |

The three stellar.toml files were retrieved on 2026-11-20.

## Why the quickstart list alone was not enough

A mainnet header is signed by whichever validator nominated the ledger's
value. In 40 consecutive-sample mainnet ledgers (65566310 to 65566583, every
seventh ledger), 21 different validators signed. Seven of them were missing
from the quickstart list. Each of the seven appears as a `[[VALIDATORS]]`
`PUBLIC_KEY` in its organization's own stellar.toml:

| Signer | Organization |
|---|---|
| `GA3NWOLW…`, `GA5MQHKW…`, `GCSXADVB…` | Range |
| `GAVEOQE4…`, `GBPBEOYM…`, `GAW3OEQR…` | MoneyGram |
| `GDSQ7BEI…` | YLDS |

With all 30 keys pinned, `ledgerwitness check-ledger --network-passphrase
"Public Global Stellar Network ; September 2015"` passes on all 40 sampled
ledgers, including the validator signature.

## Limits

- The set must follow the network. When an organization joins, leaves or
  rotates keys, its ledgers fail `validator_signature` until the list is
  updated with its source.
- One validator's signature authenticates the transaction-set hash and close
  time it signed, not a quorum decision (architecture §7).
- A stellar.toml is published by the organization over HTTPS; a copy is kept
  here because the files are not versioned upstream.
