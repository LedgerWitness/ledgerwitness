## What this changes

## Evidence

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo run -q --locked -p ledgerwitness -- verify corpus/entries`
- [ ] `make conform` (if runners, probes or corpus changed)

## Corpus changes (if any)

- [ ] Artifacts come from the recorded source; expected values come from what was submitted or an independent source, not from the decoder under test
- [ ] `validation_evidence` and `source_url_or_ledger` describe how provenance was checked
- [ ] Replay oracles bound to a changed manifest were reviewed and rebound
