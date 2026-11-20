# Every target runs offline against corpus/entries except `capture`.
LCK     := cargo run -q --locked -p ledgerwitness --
ENTRIES := corpus/entries
OUT     := results/conformance
GO      ?= go

# Current SDK releases must pass every entry. Other runs (older releases) are
# reported in the matrix but do not fail the build.
REQUIRE_PASS := --require-pass stellar-xdr \
                --require-pass github.com/stellar/go-stellar-sdk \
                --require-pass "@stellar/stellar-sdk 17.4.0" \
                --require-pass py-stellar-base

.PHONY: all verify test runners conform repro-js witnesses linter lint-wallet-backend repro-wallet-backend capture testnet-smoke testnet-evidence sweep strkey clean

all: verify test conform repro-js

verify: | $(OUT)
	$(LCK) verify $(ENTRIES) --json $(OUT)/verify.json

test:
	cargo test -q --workspace --locked
	node --test reproductions/wallet-backend/check-results.test.mjs reproductions/wallet-backend/check-results-731.test.mjs reproductions/wallet-backend/check-results-716.test.mjs reproductions/wallet-backend/check-results-716-soroban.test.mjs

$(OUT):
	mkdir -p $(OUT)

runners: | $(OUT)
	cargo run -q --locked -p ledgerwitness-runner-rust -- $(ENTRIES) > $(OUT)/rust.json
	cd runners/go && $(GO) run . ../../$(ENTRIES) > ../../$(OUT)/go.json
	cd runners/javascript && node runner.mjs ../../$(ENTRIES) --sdk 17.4.0 > ../../$(OUT)/js-17.4.0.json
	cd runners/javascript && node runner.mjs ../../$(ENTRIES) --sdk 17.2.0 > ../../$(OUT)/js-17.2.0.json
	PYTHONPATH=runners/python/.deps python3 runners/python/runner.py $(ENTRIES) > $(OUT)/python.json
	runners/java/fetch.sh && java -cp 'runners/java/.deps/*' runners/java/Runner.java $(ENTRIES) > $(OUT)/java.json

conform: runners
	$(LCK) conform $(OUT)/rust.json $(OUT)/go.json $(OUT)/js-17.4.0.json $(OUT)/js-17.2.0.json $(OUT)/python.json $(OUT)/java.json \
	  --entries $(ENTRIES) --json $(OUT)/matrix.json --markdown $(OUT)/matrix.md $(REQUIRE_PASS)

repro-js:
	cd reproductions/js-sdk && node run.mjs --json results.json

# Testnet witnesses (reproductions/sdk-findings/witnesses.json): the current
# Python, Java and JS releases must parse each applied transaction, return
# the network hash and re-encode the original bytes.
witnesses:
	PYTHONPATH=runners/python/.deps python3 reproductions/sdk-findings/check-witnesses.py
	runners/java/fetch.sh && java -cp 'runners/java/.deps/*' reproductions/sdk-findings/CheckWitnesses.java
	node reproductions/sdk-findings/check-witnesses.mjs --sdk 17.4.0
	node reproductions/sdk-findings/check-witnesses.mjs --sdk 17.2.0

# SEP-23 strkey vectors (published and rule-derived) through six decoders.
strkey:
	GO=$(GO) reproductions/strkey/run.sh

# Indexer linter prototype (rule STLR003) against its seeded cases.
linter:
	cd linter/go && $(GO) vet ./... && test -z "$$(gofmt -l .)" && $(GO) test ./...

# STLR003 on wallet-backend before and after its Soroban-participants fix and
# at the clone's HEAD: make lint-wallet-backend WALLET_BACKEND=../wallet-backend
lint-wallet-backend:
	linter/go/run-wallet-backend.sh $(WALLET_BACKEND)

# #729, #731 and #716. Needs a clone of stellar/wallet-backend:
# make repro-wallet-backend WALLET_BACKEND=../wallet-backend
repro-wallet-backend:
	reproductions/wallet-backend/run.sh $(WALLET_BACKEND)
	reproductions/wallet-backend/run-731.sh $(WALLET_BACKEND)
	reproductions/wallet-backend/run-716.sh $(WALLET_BACKEND)
	reproductions/wallet-backend/run-716-soroban.sh $(WALLET_BACKEND)

# Submits transactions to testnet with throwaway friendbot-funded keys.
capture:
	cd generators/testnet-capture && node capture-testnet.mjs out

# Live end-to-end check: capture on testnet, then verify every capture against
# the ledger hash Horizon reports. Writes to testnet; never touches mainnet.
testnet-smoke:
	generators/testnet-capture/smoke.sh

# Offline re-check of the recorded live run in docs/evidence/testnet-run/:
# ledger commitments, validator signature and the two SDK releases, against
# the raw ledgers saved at capture time. Contacts no network.
EVIDENCE := docs/evidence/testnet-run
testnet-evidence:
	cargo build -q --locked -p ledgerwitness
	@for p in $(EVIDENCE)/*.provenance.json; do \
	  shape=$$(basename $$p .provenance.json); \
	  tx=$$(node -p "require('./$$p').tx_hash"); \
	  hash=$$(node -p "require('./$$p').ledger_hash"); \
	  echo "== $$shape"; \
	  target/debug/ledgerwitness check-ledger $(EVIDENCE)/$$shape.LedgerCloseMeta.xdr \
	    --tx-hash $$tx --ledger-hash $$hash || exit 1; \
	done
	node generators/testnet-capture/sdk-check.mjs $(EVIDENCE) target/debug/ledgerwitness

# Cross-SDK sweep of public data-lake ledgers: every transaction through the
# Rust, Go, JS, Python and Java runners, compared with each other (no expected
# values).
# Downloads the ledgers; the comparison itself is offline.
#   make sweep SWEEP_LEDGERS="65566583 65566590" [SWEEP_NETWORK=pubnet]
SWEEP_NETWORK ?= pubnet
SWEEP_DIR ?= results/sweep
sweep: | $(OUT)
	rm -rf $(SWEEP_DIR) && mkdir -p $(SWEEP_DIR)
	generators/datalake-sweep/build.sh $(SWEEP_DIR)/set $(SWEEP_NETWORK) $(SWEEP_LEDGERS)
	cargo build -q --release --locked -p ledgerwitness-runner-rust
	target/release/ledgerwitness-runner-rust $(abspath $(SWEEP_DIR)/set) > $(SWEEP_DIR)/rust.json
	cd runners/go && $(GO) run . $(abspath $(SWEEP_DIR)/set) > $(abspath $(SWEEP_DIR))/go.json
	cd runners/javascript && node runner.mjs $(abspath $(SWEEP_DIR)/set) --sdk 17.4.0 > $(abspath $(SWEEP_DIR))/js.json
	PYTHONPATH=runners/python/.deps python3 runners/python/runner.py $(SWEEP_DIR)/set > $(SWEEP_DIR)/python.json
	runners/java/fetch.sh && java -Xmx6g -cp 'runners/java/.deps/*' runners/java/Runner.java $(SWEEP_DIR)/set > $(SWEEP_DIR)/java.json
	node generators/datalake-sweep/compare.mjs $(SWEEP_DIR)/rust.json $(SWEEP_DIR)/go.json $(SWEEP_DIR)/js.json $(SWEEP_DIR)/python.json $(SWEEP_DIR)/java.json --json $(SWEEP_DIR)/compare.json

clean:
	rm -rf target results
