# Curated replay oracles

These three sidecars use [replay comparison format 0.1.0](../../docs/spec/replay.md):
the two signed-payload signer entries and the SAC-created trustline entry.
They reuse the participant and signer/trustline expectations already curated
in each entry's `expected_state_changes_or_participants`; they do not derive
new expected effects from the decoder or supply adapter observations.

Participant assertions use subset matching. Signer/trustline collections use
exact keyed-row matching within the focus transaction's operation effects.
Bookkeeping effects and unrelated transactions belong outside that scope.
Full P… signer strings, trustline flags and the decimal-string i64 limit
must survive normalization unchanged.

Manifest and ledger hashes bind the sidecars to exact corpus revisions.
An intentional manifest/fixture change requires reviewing the expectations
and refreshing the sidecar binding. CLI tests check these bindings and their
links to existing expectations. The muxed payment and LP-trustline entries
have no replay oracles yet; these three are not full-corpus replay coverage.

No committed observation from a real database is included. The existing
wallet-backend #729 conversion reports do not establish a database commit
and must not be relabelled as passing persisted observations.
