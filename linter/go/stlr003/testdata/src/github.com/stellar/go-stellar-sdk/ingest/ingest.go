// Package ingest is a minimal stand-in for go-stellar-sdk/ingest.
package ingest

import "github.com/stellar/go-stellar-sdk/xdr"

type LedgerTransaction struct {
	Envelope   xdr.TransactionEnvelope
	UnsafeMeta xdr.TransactionMeta
	FeeChanges xdr.LedgerEntryChanges
}

type Change struct{ Sponsor string }

func (t *LedgerTransaction) GetOperationChanges(index uint32) ([]Change, error) { return nil, nil }
