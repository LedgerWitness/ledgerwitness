# Test data

`pubnet-18000000.xdr`: mainnet ledger 18000000 (protocol 9) as
LedgerCloseMeta v0, from the public data lake object
`v1.1/stellar/ledgers/pubnet/FEED95FF--17984000-18047999/FEED577F--18000000.xdr.zst`,
decompressed with `ledgerwitness galexie-extract`. Its header hash,
`713a9e0464674bc646d234afbb9df640185053156aee9fa9cd72069921f70f9c`, is the
one SDF's history archive records for the ledger
(`generators/datalake-sweep/archive-hash.mjs 18000000`). It is test data for
the legacy transaction set, not a corpus entry: the header's `StellarValue` is
unsigned, so it cannot be attested.
