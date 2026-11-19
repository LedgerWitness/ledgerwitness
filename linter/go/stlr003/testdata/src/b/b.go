// A package with a type error unrelated to participants, as when a cgo
// dependency cannot be built: the rule must still analyse the rest.
package b

import "github.com/stellar/go-stellar-sdk/ingest"

func decodeEvent() string { return undefinedDecoder() }

func payerParticipants(tx ingest.LedgerTransaction) []string { // want `STLR003: payerParticipants derives participants`
	return []string{tx.Envelope.SourceAccount().Address()}
}
