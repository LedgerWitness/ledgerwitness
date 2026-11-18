# Contributing

## Setup

- Rust: the version in `rust-toolchain.toml` (installed automatically by rustup)
- Go: 1.25 or later (`runners/go/go.mod`)
- Node.js: 22
- `make`

```sh
npm ci --prefix runners/javascript
npm ci --prefix reproductions/js-sdk
make all        # verify, test, conform, repro-js
```

Everything except `make capture` runs offline against `corpus/entries`.

## Before opening a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -q --locked -p ledgerwitness -- verify corpus/entries
make conform            # when runners, probes or the corpus change
```

Go changes: `gofmt -l runners/go` must print nothing and `go vet ./...` must
pass. Keep changes focused; one concern per pull request.

## Adding a corpus entry

1. Obtain the artifact from a recorded source: the Galexie data lake
   (`ledgerwitness galexie-extract`), RPC `getLedgers` (raw base64, not an SDK's
   re-encoding), or `generators/testnet-capture`. Never hand-edit `.xdr`
   files.
2. Create `corpus/entries/<class>/<id>/` with the artifact and
   `focus-envelope.xdr` (`ledgerwitness extract-envelope`).
3. Write `entry.json` against `corpus/schema/manifest.schema.json`. Expected
   values must come from what was submitted, from an independent service, or
   from the CAP text, never from the decoder under test. Record how in
   `validation_evidence`.
4. Record the ledger hash from a source other than the artifact (for example
   Horizon `/ledgers/{seq}`). Core-emitted entries must pass
   `validator_signature`, so they need a network whose validators are pinned
   in `crates/ledgerwitness/src/attest.rs` (today: testnet). Check a fresh capture first
   with `ledgerwitness check-ledger <ledger.xdr> --tx-hash … --ledger-hash …`.
5. Add the CAP to `docs/research/coverage-registry.md` if it is new.
6. `ledgerwitness verify` must pass, and `make conform` must still pass for current SDKs.

`synthetic_validated` entries also need verified evidence that stellar-core
can emit the shape; `sdk_negative_only` entries are never counted as ledger
coverage. See `corpus/README.md`.

## Changing pinned SDK versions

The runners pin current SDK releases; `runners/javascript` and
`reproductions/js-sdk` also pin historical releases on purpose. When bumping a
current pin, rerun `make conform` and update the matrix in the README. Do not
bump historical pins: they define the pre-fix side of a reproduction.

## Security

Do not open public issues for vulnerabilities or for undisclosed defects
found in Stellar software. See `SECURITY.md`.

## License

By contributing you agree that your contributions are licensed under
Apache-2.0 (`LICENSE`).
