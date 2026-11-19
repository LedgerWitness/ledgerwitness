// Storage round-trip harness for wallet-backend#716 (signed-payload signer
// producer), copied into internal/indexer of a wallet-backend checkout by
// run-716.sh. It runs the checkout's own indexer on one corpus ledger
// (indexer.ProcessLedger, the call live ingestion makes) and puts every
// signer address it produced through the signer column's own database
// conversion: Value() as an INSERT would, then Scan() of those bytes as a
// SELECT would. No database is needed; the column type does the conversion.
//
// The column type changed across the commits under test (NullAddressBytea,
// later NullSignerKeyBytea), so it is driven through the database/sql
// interfaces rather than by name.
//
// Environment:
//   LW_LEDGER      path to a LedgerCloseMeta XDR file
//   LW_PASSPHRASE  network passphrase of that ledger
//   LW_REPORT      path the JSON report is written to
package indexer_test

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"testing"

	"github.com/alitto/pond/v2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go-stellar-sdk/strkey"
	"github.com/stellar/go-stellar-sdk/xdr"

	"github.com/stellar/wallet-backend/internal/indexer"
	"github.com/stellar/wallet-backend/internal/metrics"
)

type roundTrip struct {
	Signer        string `json:"signer"`
	ColumnType    string `json:"column_type"`
	WriteError    string `json:"write_error,omitempty"`
	StoredHex     string `json:"stored_hex,omitempty"`
	StoredBytes   int    `json:"stored_bytes"`
	ReadBack      string `json:"read_back,omitempty"`
	ReadBackError string `json:"read_back_decode_error,omitempty"`
	Exact         bool   `json:"exact"`
}

type roundTripReport struct {
	Ledger       uint32      `json:"ledger"`
	Transactions int         `json:"transactions"`
	StateChanges int         `json:"state_changes"`
	Signers      []roundTrip `json:"signer_round_trips"`
}

func TestLWSignerRoundTrip(t *testing.T) {
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

	rep := roundTripReport{Ledger: lcm.LedgerSequence(), Transactions: lcm.CountTransactions()}
	for _, sc := range buf.GetStateChanges() {
		rep.StateChanges++
		col := sc.SignerAccountID
		signer := fmt.Sprint(col)
		if signer == "" {
			continue
		}
		r := roundTrip{Signer: signer, ColumnType: reflect.TypeOf(col).Name()}
		stored, err := any(col).(driver.Valuer).Value()
		if err != nil {
			r.WriteError = err.Error()
			rep.Signers = append(rep.Signers, r)
			continue
		}
		b, _ := stored.([]byte)
		r.StoredHex, r.StoredBytes = hex.EncodeToString(b), len(b)

		fresh := reflect.New(reflect.TypeOf(col))
		if err := fresh.Interface().(sql.Scanner).Scan(stored); err != nil {
			r.ReadBackError = "scan: " + err.Error()
			rep.Signers = append(rep.Signers, r)
			continue
		}
		r.ReadBack = fmt.Sprint(fresh.Elem().Interface())
		if _, _, err := strkey.DecodeAny(r.ReadBack); err != nil {
			r.ReadBackError = err.Error()
		}
		r.Exact = r.ReadBack == signer
		rep.Signers = append(rep.Signers, r)
	}

	b, _ := json.MarshalIndent(rep, "", "  ")
	if err := os.WriteFile(out, append(b, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
}
