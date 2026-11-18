# Every target runs offline against corpus/entries except `capture`.
LCK     := cargo run -q --locked -p ledgerwitness --
ENTRIES := corpus/entries
OUT     := results/conformance
GO      ?= go

# Current SDK releases must pass every entry. Other runs (older releases) are
# reported in the matrix but do not fail the build.
REQUIRE_PASS := --require-pass stellar-xdr \
                --require-pass github.com/stellar/go-stellar-sdk \
                --require-pass "@stellar/stellar-sdk 17.4.0"

.PHONY: all verify test runners conform repro-js repro-wallet-backend capture testnet-smoke testnet-evidence clean

all: verify test conform repro-js

verify: | $(OUT)
	$(LCK) verify $(ENTRIES) --json $(OUT)/verify.json

test:
	cargo test -q --workspace --locked
	node --test reproductions/wallet-backend/check-results.test.mjs

$(OUT):
	mkdir -p $(OUT)

runners: | $(OUT)
	cargo run -q --locked -p ledgerwitness-runner-rust -- $(ENTRIES) > $(OUT)/rust.json
	cd runners/go && $(GO) run . ../../$(ENTRIES) > ../../$(OUT)/go.json
	cd runners/javascript && node runner.mjs ../../$(ENTRIES) --sdk 17.4.0 > ../../$(OUT)/js-17.4.0.json
	cd runners/javascript && node runner.mjs ../../$(ENTRIES) --sdk 17.2.0 > ../../$(OUT)/js-17.2.0.json

conform: runners
	$(LCK) conform $(OUT)/rust.json $(OUT)/go.json $(OUT)/js-17.4.0.json $(OUT)/js-17.2.0.json \
	  --entries $(ENTRIES) --json $(OUT)/matrix.json --markdown $(OUT)/matrix.md $(REQUIRE_PASS)

repro-js:
	cd reproductions/js-sdk && node run.mjs --json results.json

# Needs a clone of stellar/wallet-backend: make repro-wallet-backend WALLET_BACKEND=../wallet-backend
repro-wallet-backend:
	reproductions/wallet-backend/run.sh $(WALLET_BACKEND)

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

clean:
	rm -rf target results
