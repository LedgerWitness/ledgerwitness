# sdk-negative

Invalid or non-canonical inputs used to test SDK validation: out-of-range IDs,
bad union discriminants, over-length vectors, non-canonical encodings. These
are never counted as ledger coverage. Each entry's `expected_outcome` states
whether a correct decoder must reject it on decode or on validation.

No entries yet. The SDK-negative cases for js-stellar-sdk #1785 currently live
in `reproductions/js-sdk/`, because they are API inputs rather than XDR.
