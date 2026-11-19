// Participant storage harness for wallet-backend#716 (muxed Soroban operation
// source producer), copied into internal/indexer of a wallet-backend checkout
// by run-716.sh. It runs the checkout's own indexer on one corpus ledger and
// puts every operation participant recorded for one account through the
// participant column type's own database conversion: Value() as an INSERT
// would, then Scan() of those bytes as a SELECT would.
//
// indexer.ProcessLedger returns (int, error) at some commits under test and
// (int, []ingest.LedgerTransaction, error) at others, so it is called through
// reflection and its last result is taken as the error.
//
// Environment:
//   LW_LEDGER      path to a LedgerCloseMeta XDR file
//   LW_PASSPHRASE  network passphrase of that ledger
//   LW_ACCOUNT     base G... account whose operation participants are reported
//   LW_REPORT      path the JSON report is written to
package indexer_test

import (
	"context"
	"encoding/json"
	"os"
	"reflect"
	"sort"
	"testing"

	"github.com/alitto/pond/v2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go-stellar-sdk/strkey"
	"github.com/stellar/go-stellar-sdk/xdr"

	"github.com/stellar/wallet-backend/internal/indexer"
	"github.com/stellar/wallet-backend/internal/indexer/types"
	"github.com/stellar/wallet-backend/internal/metrics"
)

type participantRoundTrip struct {
	OperationID   int64  `json:"operation_id"`
	Participant   string `json:"participant"`
	WriteError    string `json:"write_error,omitempty"`
	StoredBytes   int    `json:"stored_bytes"`
	ReadBack      string `json:"read_back,omitempty"`
	ReadBackError string `json:"read_back_decode_error,omitempty"`
	Exact         bool   `json:"exact"`
}

type participantReport struct {
	Ledger       uint32                 `json:"ledger"`
	Account      string                 `json:"account"`
	Participants []participantRoundTrip `json:"operation_participants"`
}

// sameAccount reports whether address is account itself or a muxed form of it.
func sameAccount(address, account string) bool {
	if address == account {
		return true
	}
	muxed, err := xdr.AddressToMuxedAccount(address)
	return err == nil && muxed.ToAccountId().Address() == account
}

func TestLWParticipantRoundTrip(t *testing.T) {
	path, passphrase := os.Getenv("LW_LEDGER"), os.Getenv("LW_PASSPHRASE")
	account, out := os.Getenv("LW_ACCOUNT"), os.Getenv("LW_REPORT")
	if path == "" || passphrase == "" || account == "" || out == "" {
		t.Skip("LW_LEDGER, LW_PASSPHRASE, LW_ACCOUNT and LW_REPORT must be set")
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
	results := reflect.ValueOf(indexer.ProcessLedger).Call([]reflect.Value{
		reflect.ValueOf(context.Background()), reflect.ValueOf(passphrase),
		reflect.ValueOf(lcm), reflect.ValueOf(idx), reflect.ValueOf(buf),
	})
	if e := results[len(results)-1]; !e.IsNil() {
		t.Fatalf("indexer.ProcessLedger: %v", e.Interface())
	}

	rep := participantReport{Ledger: lcm.LedgerSequence(), Account: account}
	for opID, set := range buf.GetOperationsParticipants() {
		for participant := range set {
			if !sameAccount(participant, account) {
				continue
			}
			r := participantRoundTrip{OperationID: opID, Participant: participant}
			stored, err := types.AddressBytea(participant).Value()
			if err != nil {
				r.WriteError = err.Error()
				rep.Participants = append(rep.Participants, r)
				continue
			}
			b, _ := stored.([]byte)
			r.StoredBytes = len(b)
			var back types.AddressBytea
			if err := back.Scan(stored); err != nil {
				r.ReadBackError = "scan: " + err.Error()
			} else {
				r.ReadBack = back.String()
				if _, _, err := strkey.DecodeAny(r.ReadBack); err != nil {
					r.ReadBackError = err.Error()
				}
				r.Exact = r.ReadBack == participant
			}
			rep.Participants = append(rep.Participants, r)
		}
	}
	sort.Slice(rep.Participants, func(i, j int) bool {
		a, b := rep.Participants[i], rep.Participants[j]
		return a.OperationID < b.OperationID || (a.OperationID == b.OperationID && a.Participant < b.Participant)
	})

	b, _ := json.MarshalIndent(rep, "", "  ")
	if err := os.WriteFile(out, append(b, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
}
