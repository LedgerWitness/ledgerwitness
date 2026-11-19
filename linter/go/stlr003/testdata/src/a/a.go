// Seeded cases for STLR003. Each "want" comment is a finding the rule must
// report; every other function must stay silent.
package a

import (
	"github.com/stellar/go-stellar-sdk/ingest"
	"github.com/stellar/go-stellar-sdk/xdr"
)

// Envelope only: a payment destination is whatever the submitter wrote.
func paymentParticipants(tx ingest.LedgerTransaction) []string { // want `STLR003: paymentParticipants derives participants from the transaction envelope`
	var out []string
	for _, op := range tx.Envelope.Operations() {
		out = append(out, op.Body.MustPaymentOp().Destination.Address())
	}
	return out
}

// Envelope only, through a same-package helper: the authorization-tree walk
// wallet-backend replaced in 9538b3ca (wallet-backend#753).
func authParticipants(tx ingest.LedgerTransaction) []string { // want `STLR003: authParticipants derives participants`
	return walkAuth(tx.Envelope.Operations()[0].Body.MustInvokeHostFunctionOp().Auth)
}

func walkAuth(entries []xdr.SorobanAuthorizationEntry) []string {
	var out []string
	for _, e := range entries {
		out = append(out, e.Credentials.Address())
	}
	return out
}

// A method is a candidate too.
type opView struct{ tx *ingest.LedgerTransaction }

func (v opView) Participants() []string { // want `STLR003: Participants derives participants`
	return []string{v.tx.Envelope.SourceAccount().Address()}
}

// Meta read directly: not reported, even though the envelope is read too.
func metaParticipants(tx ingest.LedgerTransaction) []string {
	out := []string{tx.Envelope.SourceAccount().Address()}
	for _, c := range tx.FeeChanges {
		out = append(out, c.Account())
	}
	return out
}

// Meta through helpers, in the shape of wallet-backend's
// TransactionOperationWrapper.Participants -> getSponsor -> Changes.
type wrapper struct {
	tx    *ingest.LedgerTransaction
	index uint32
}

func (w *wrapper) changes() ([]ingest.Change, error) { return w.tx.GetOperationChanges(w.index) }

func (w *wrapper) sponsor() string {
	cs, _ := w.changes()
	if len(cs) > 0 {
		return cs[0].Sponsor
	}
	return ""
}

func (w *wrapper) Participants() []string {
	return []string{w.tx.Envelope.SourceAccount().Address(), w.sponsor()}
}

// Not a participant builder: reading the envelope is fine.
func sourceAccount(tx ingest.LedgerTransaction) string {
	return tx.Envelope.SourceAccount().Address()
}

// Neither input: nothing to judge.
func dedupeParticipants(in []string) []string { return in }

// Suppressed with a reason.
//
//lint:ignore STLR003 the fee-bump source pays the fee, so it is always affected
func feeBumpParticipants(tx ingest.LedgerTransaction) []string {
	return []string{tx.Envelope.FeeBumpAccount().Address()}
}
