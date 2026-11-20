# Evidence: strkey decoders against SEP-23

Every indexer decodes `G…`, `M…`, `P…` and the other SEP-23 strkeys, and two
decoders that disagree on one string disagree about which account it names.
`make strkey` (`reproductions/strkey/`) puts 144 strkeys through six
decoders and scores them against SEP-23.

## Vectors

| Set | Count | Source |
|---|---|---|
| Published | 34 (16 valid, 18 invalid) | SEP-23's own test cases, stellar/stellar-protocol@f2d364902615 |
| Rule-derived | 110 (54 valid controls, 56 invalid) | `craft.py`, an encoder written from SEP-23's specification: `P…` keys with non-zero unused trailing bits for every payload length that has them (SEP-23: "unused bits of the last symbol must be zero"; its own vectors test this only for `M`, `W` and `B`), lowercase input and `=` padding (both fail SEP-23's rule that a strkey is valid only if it re-encodes to the same string), a 65-byte payload (above XDR `opaque<64>`), and an empty payload (not specified by SEP-23) |

An invalid vector must be rejected; a valid one must be accepted and
re-encode to the same string.

## Result

| Decoder | Correct | Wrong | Not implemented |
|---|---|---|---|
| Rust stellar-strkey 1.0.0 | 143 | 0 | 0 |
| Go go-stellar-sdk v0.9.0 | 135 | **1, already reported upstream** | 7 (`W…` muxed contracts) |
| JS @stellar/stellar-sdk 17.4.0 | 143 | 0 | 0 |
| JS @stellar/stellar-sdk 17.2.0 | 136 | 0 | 7 (`W…`) |
| Python py-stellar-base 17.0.1 | 143 | 0 | 0 |
| Java java-stellar-sdk 6.0.0 | 143 | 0 | 0 |

The empty-payload `P…` key, which SEP-23 does not cover, is rejected by all
six.

The one wrong verdict: Go's `strkey.Decode` and `strkey.IsValidClaimableBalance`
accept SEP-23's invalid vector "invalid claimable balance type (first byte of
binary key is not 0)"; `xdr.ClaimableBalanceId.DecodeFromStrkey` rejects it.
It is reported as stellar/go-stellar-sdk#6042 and #6046, so it is listed in
`known.json` and does not fail the run; the run fails if it stops
reproducing, so the upstream fix shows up.

## What this shows

No new defect: the decoders follow SEP-23 on every rule tested, including the
cases SEP-23's own list leaves out. This is a negative result, and it is
recorded because it narrows where defects remain: a strkey divergence between
these SDKs is unlikely, and the corpus's harder cases lie elsewhere.

## Limits

- Six decoders. The PHP, Dart, .NET and other SDKs are not tested here.
- Java's decoders are strict, as java-stellar-sdk#836 also reports. Its
  encoders accept data of the wrong length; that is #836's subject and is not
  tested here.
- The decoders tested are the StrKey functions; wrappers built on them (for
  example account or keypair constructors) were checked by hand for
  lowercase input in JS and Python, and reject it.
