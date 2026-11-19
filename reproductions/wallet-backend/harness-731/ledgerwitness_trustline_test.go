// Trustline replay harness for wallet-backend#731, copied into
// internal/indexer of a wallet-backend checkout by run-731.sh. It runs the
// checkout's own indexer on one corpus ledger (indexer.ProcessLedger, the
// call live ingestion makes) and reports, side by side:
//   - every trustline the ledger meta shows core created, per operation;
//   - every TRUSTLINE state change the indexer produced.
// A trustline created by the Stellar Asset Contract's trust function
// (CAP-0073) with no TRUSTLINE/ADD state change is the #731 defect.
//
// Environment:
//   LW_LEDGER      path to a LedgerCloseMeta XDR file
//   LW_PASSPHRASE  network passphrase of that ledger
//   LW_REPORT      path the JSON report is written to
package indexer_test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"testing"

	"github.com/alitto/pond/v2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go-stellar-sdk/xdr"

	"github.com/stellar/wallet-backend/internal/indexer"
	"github.com/stellar/wallet-backend/internal/indexer/types"
	"github.com/stellar/wallet-backend/internal/metrics"
)

type createdTrustline struct {
	TxHash  string `json:"tx_hash"`
	Account string `json:"account"`
	Asset   string `json:"asset"`
}

type trustlineChange struct {
	Account string `json:"account"`
	Reason  string `json:"reason"`
	Token   string `json:"token,omitempty"`
}

type trustlineReport struct {
	Ledger            uint32             `json:"ledger"`
	Transactions      int                `json:"transactions"`
	StateChanges      int                `json:"state_changes"`
	CreatedInMeta     []createdTrustline `json:"trustlines_created_in_meta"`
	TrustlineChanges  []trustlineChange  `json:"trustline_state_changes"`
	CreatedWithoutAdd []createdTrustline `json:"created_without_trustline_add"`
}

func assetString(a xdr.TrustLineAsset) string {
	switch a.Type {
	case xdr.AssetTypeAssetTypePoolShare:
		return "pool_share"
	default:
		return a.ToAsset().StringCanonical()
	}
}

func TestLWTrustlines(t *testing.T) {
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
	_, txs, err := indexer.ProcessLedger(context.Background(), passphrase, lcm, idx, buf)
	if err != nil {
		t.Fatalf("indexer.ProcessLedger: %v", err)
	}

	rep := trustlineReport{Ledger: lcm.LedgerSequence(), Transactions: len(txs)}
	for _, tx := range txs {
		if !tx.Result.Successful() {
			continue
		}
		ops := tx.Envelope.Operations()
		for i := range ops {
			changes, err := tx.GetOperationChanges(uint32(i))
			if err != nil {
				t.Fatal(err)
			}
			for _, c := range changes {
				if c.Type != xdr.LedgerEntryTypeTrustline || c.Pre != nil || c.Post == nil {
					continue
				}
				tl := c.Post.Data.MustTrustLine()
				rep.CreatedInMeta = append(rep.CreatedInMeta, createdTrustline{
					TxHash:  tx.Hash.HexString(),
					Account: tl.AccountId.Address(),
					Asset:   assetString(tl.Asset),
				})
			}
		}
	}

	added := map[string]bool{}
	for _, sc := range buf.GetStateChanges() {
		rep.StateChanges++
		if sc.StateChangeCategory != types.StateChangeCategoryTrustline {
			continue
		}
		account := sc.AccountID.String()
		rep.TrustlineChanges = append(rep.TrustlineChanges, trustlineChange{
			Account: account,
			Reason:  fmt.Sprint(sc.StateChangeReason),
			Token:   sc.TokenID.String(),
		})
		if sc.StateChangeReason == types.StateChangeReasonAdd {
			added[account] = true
		}
	}
	for _, c := range rep.CreatedInMeta {
		if c.Asset != "pool_share" && !added[c.Account] {
			rep.CreatedWithoutAdd = append(rep.CreatedWithoutAdd, c)
		}
	}
	sort.Slice(rep.TrustlineChanges, func(i, j int) bool {
		a, b := rep.TrustlineChanges[i], rep.TrustlineChanges[j]
		return a.Account+a.Reason+a.Token < b.Account+b.Reason+b.Token
	})

	b, _ := json.MarshalIndent(rep, "", "  ")
	if err := os.WriteFile(out, append(b, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
}
