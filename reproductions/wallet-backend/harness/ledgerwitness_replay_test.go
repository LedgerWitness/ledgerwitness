// Corpus replay harness for wallet-backend, copied into internal/data of a
// wallet-backend checkout by run.sh. It runs the checkout's own indexer on one
// corpus ledger (the same indexer.ProcessLedger call live ingestion makes)
// and then converts every resulting state change into the values the
// state_changes COPY sends to Postgres, using the checkout's own conversion
// code (exposed by a per-commit shim, see shims/). No database is needed.
//
// Environment:
//   LW_LEDGER      path to a LedgerCloseMeta XDR file
//   LW_PASSPHRASE  network passphrase of that ledger
//   LW_REPORT      path the JSON report is written to
package data_test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"testing"

	"github.com/alitto/pond/v2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go-stellar-sdk/xdr"

	"github.com/stellar/wallet-backend/internal/data"
	"github.com/stellar/wallet-backend/internal/indexer"
	"github.com/stellar/wallet-backend/internal/metrics"
)

type rowResult struct {
	ToID     int64  `json:"to_id"`
	Category string `json:"category"`
	Reason   string `json:"reason"`
	Signer   string `json:"signer,omitempty"`
	Encoded  bool   `json:"encoded"`
	Error    string `json:"error,omitempty"`
}

type report struct {
	Ledger            uint32      `json:"ledger"`
	Transactions      int         `json:"transactions"`
	StateChanges      int         `json:"state_changes"`
	RowsFailingEncode int         `json:"rows_failing_encode"`
	SignerRows        []rowResult `json:"signer_rows"`
	FirstEncodeError  string      `json:"first_encode_error,omitempty"`
}

func TestLWReplay(t *testing.T) {
	path, passphrase, out := os.Getenv("LW_LEDGER"), os.Getenv("LW_PASSPHRASE"), os.Getenv("LW_REPORT")
	if path == "" || passphrase == "" || out == "" {
		t.Skip("LW_LEDGER, LW_PASSPHRASE and LW_REPORT must be set")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var lcm xdr.LedgerCloseMeta
	if err := xdr.SafeUnmarshal(raw, &lcm); err != nil {
		t.Fatalf("decoding ledger: %v", err)
	}

	m := metrics.NewMetrics(prometheus.NewRegistry())
	idx, err := indexer.NewIndexer(passphrase, pond.NewPool(4), m.Ingestion)
	if err != nil {
		t.Fatal(err)
	}
	buf := indexer.NewIndexerBuffer()
	if _, err := indexer.ProcessLedger(context.Background(), passphrase, lcm, idx, buf); err != nil {
		t.Fatalf("indexer.ProcessLedger: %v", err)
	}

	rep := report{Ledger: lcm.LedgerSequence(), Transactions: lcm.CountTransactions()}
	for _, sc := range buf.GetStateChanges() {
		rep.StateChanges++
		err := data.EncodeStateChangeForCopy(sc)
		if err != nil {
			rep.RowsFailingEncode++
			if rep.FirstEncodeError == "" {
				rep.FirstEncodeError = err.Error()
			}
		}
		if signer := sc.SignerAccountID.String(); signer != "" {
			r := rowResult{
				ToID:     sc.ToID,
				Category: fmt.Sprint(sc.StateChangeCategory),
				Reason:   fmt.Sprint(sc.StateChangeReason),
				Signer:   signer,
				Encoded:  err == nil,
			}
			if err != nil {
				r.Error = err.Error()
			}
			rep.SignerRows = append(rep.SignerRows, r)
		}
	}
	b, _ := json.MarshalIndent(rep, "", "  ")
	if err := os.WriteFile(out, append(b, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
}
