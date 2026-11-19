# Indexer-hardening guide (draft)

**Status: draft, not reviewed.** This is the first part of the guide planned
in [architecture §14](../architecture.md). Each rule below is checked against
stellar-core's source at commit [`35bd0f9f66`](https://github.com/stellar/stellar-core/tree/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692)
and, where the corpus has one, linked to the entry that exercises it.
Publication needs review by a stellar-core or platform engineer; until then,
treat it as a working document.

Two principles run through it:

1. **Trust only what stellar-core checks.** Core validates some fields of a
   transaction and passes others through untouched. A field core does not
   check can hold anything a submitter wants.
2. **Take what happened from the meta, not the envelope.** The envelope is
   what the submitter claimed; the ledger-entry changes and events in the
   meta are what core applied.

## 1. What stellar-core checks, field by field

"XDR" means the limit comes only from the XDR type: core accepts any value
that decodes. "Core" means core applies a rule beyond that.

| Field | What is checked | Source | What an indexer must do | Corpus |
|---|---|---|---|---|
| Memo text (`MEMO_TEXT`) | **XDR only:** at most 28 bytes. The content is never checked; non-UTF-8 bytes and NULs are valid. From protocol 25, a transaction whose single operation is InvokeHostFunction may carry no memo at all. In a CAP-67 transfer event whose destination is not muxed, core puts the transaction's memo, text included and unchanged, in the `to_muxed_id` field. | [`validateSorobanMemo`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/TransactionFrame.cpp#L342), [transfer event](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/EventManager.cpp#L131), [`makeClassicMemoSCVal`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/TransactionUtils.cpp#L2253) | Store memo text as bytes. Decode to text only for display, without failing on invalid UTF-8. Do not assume an event's `to_muxed_id` is a number: it can be a memo string, an ID or a hash. In six recent mainnet ledgers, 812 of the 814 `to_muxed_id` values in transfer events were memo text (810) or memo hashes (2), all from classic operations ([evidence](../evidence/mainnet-sweep/README.md)). | not yet |
| Muxed account ID (`M…`) | **XDR only:** any unsigned 64-bit value, as transaction source, operation source or payment destination. Core rejects a muxed source only on a transaction whose operation is InvokeHostFunction (protocol 25+); ExtendFootprintTTL and RestoreFootprint accept it. | [`validateSorobanMemo`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/TransactionFrame.cpp#L342) | Keep the ID as an unsigned 64-bit value (a decimal string in JSON). Store the full `M…` address or the (`G…`, ID) pair, never a 32-byte truncation. Attribute participation to the base `G…` account. | [muxed payment](../../corpus/entries/core-emitted/core.testnet.cap27.muxed-large-ids-payment.p30/entry.json), [muxed ExtendFootprintTTL](../../corpus/entries/core-emitted/core.testnet.cap27.soroban-muxed-extend-ttl.p30/entry.json) |
| Signed-payload signer (`P…`) | **Core:** the payload must not be empty, both for an account signer added by SetOptions and for a transaction's extra signer. **XDR:** at most 64 payload bytes. | [SetOptions](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/SetOptionsOpFrame.cpp#L309), [extra signers](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/TransactionFrame.cpp#L1289) | Store the whole signer key: 32 key bytes, a 4-byte length and the padded payload. A column sized for 32-byte account keys truncates it (wallet-backend#716, #729). | [#729 ledger](../../corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/entry.json), [padded payload](../../corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer-padded.p30/entry.json) |
| Home domain (SetOptions) | **Core:** printable ASCII only (`isStringValid`). **XDR:** at most 32 bytes. | [SetOptions](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/SetOptionsOpFrame.cpp#L320), [`isStringValid`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/util/types.cpp#L106) | Can be stored as ASCII text. | not yet |
| Data entry name (ManageData) | **Core:** printable ASCII only. **XDR:** at most 64 bytes. | [ManageData](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/ManageDataOpFrame.cpp#L115) | Can be stored as ASCII text. | not yet |
| Data entry value (ManageData) | **XDR only:** up to 64 arbitrary bytes. | [ManageData](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/ManageDataOpFrame.cpp#L115) (checks the name only) | Store as bytes; never assume text. | not yet |
| Asset code | **Core:** ASCII letters and digits only, zero padding only at the end; at least 1 character for a 4-byte code. | [`isAssetValid`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/util/types.cpp#L149) | Asset codes in accepted operations can be stored as ASCII text after removing trailing zeros. | all trustline and payment entries |
| Fee-bump fees | **Core:** the outer inclusion fee must meet the minimum and, when the inner fee is not negative, outbid the inner fee rate. The inner fee itself may be 0. | [`FeeBumpTransactionFrame`](https://github.com/stellar/stellar-core/blob/35bd0f9f66d3a1e5fe04ef34b32fc6ba81340692/src/transactions/FeeBumpTransactionFrame.cpp#L401) | Use the outer hash as the transaction ID and the inner source as the transaction source; charge fees to the fee source. Do not assume an inner fee at or above the minimum. | [mainnet fee-bump](../../corpus/entries/core-emitted/core.pubnet.cap15.fee-bump-zero-inner-fee.p29/entry.json) |

### Still to verify

These belong in the table but are not yet checked against core's source;
they are listed here so they are not mistaken for verified rules:

- Soroban authorization entries that no `require_auth` call matched: which
  parts the host checks, and the depth limits on authorization trees and
  nested ScVals at admission.
- Contract event contents and topics beyond the events core itself emits.
- Offer IDs and other 64-bit identifiers that JavaScript numbers cannot hold.

## 2. Participants come from the meta

An account took part in a transaction if core changed it, not because the
envelope names it.

- **Do:** derive participants from the ledger-entry changes and events of
  each operation, and from the fee and sequence changes of the transaction.
- **Do not:** take participants from the authorization tree, payment
  destinations of failed transactions, or any other envelope field alone.
  Anyone can name any account in a transaction they submit, at the cost of a
  base fee.

Evidence:

- wallet-backend replaced its authorization-tree participants with the nonce
  entries core writes after a signature verifies (`9538b3ca`, #753). Linter
  rule STLR003 flags the six envelope-only functions it replaced
  ([evidence](../evidence/linter-stlr003/README.md)).
- Indexers differ on failed operations: wallet-backend credits only the
  source, Horizon credits every named account and shows them with
  `include_failed=true` ([OBS-003](../findings/README.md)).

## 3. Trustlines and accounts created without their usual operation

- A Stellar Asset Contract `trust` call creates a trustline with no
  ChangeTrust operation and no contract event (CAP-0073). Read trustline
  creation from the ledger-entry changes
  ([corpus entry](../../corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30/entry.json),
  wallet-backend#731).
- Pool shares: the pool ID is the hash of the asset pair in XDR byte order,
  not strkey text order (CAP-0038,
  [corpus entry](../../corpus/entries/core-emitted/core.testnet.cap38.lp-trustline-issuer-order.p30/entry.json),
  js-stellar-sdk#1766).

## 4. Checklist

For an indexer in any language. The first three items mirror the linter rules
of [architecture §13](../architecture.md); only STLR003 is implemented so
far.

- [ ] No field core does not check (memo text, data values, muxed IDs, event
      data) is used as a database key, identity or trusted text (STLR001).
- [ ] Every recursive walk over authorization trees, ScVals or nested XDR has
      an explicit depth and size limit (STLR002).
- [ ] Participants are derived from the meta, not the envelope (STLR003).
- [ ] Muxed IDs are stored as unsigned 64-bit values; muxed and
      signed-payload keys are never truncated to 32 bytes.
- [ ] Fee-bump transactions are keyed by their outer hash.
- [ ] The indexer passes `make conform` and `make sweep` style checks on the
      corpus for the SDK it uses.

## 5. Status and next steps

- Table rows are linked to corpus entries by hand. A CI check that every
  row's corpus link exists and every corpus shape tag maps to a row is
  planned (architecture §21, milestone 3).
- Recommended numeric depth and size limits will be measured against core's
  admission limits before they are published.
