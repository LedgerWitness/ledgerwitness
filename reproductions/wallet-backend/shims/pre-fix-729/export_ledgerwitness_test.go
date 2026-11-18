package data

import "github.com/stellar/wallet-backend/internal/indexer/types"

// EncodeStateChangeForCopy runs the conversion that StateChangeModel.BatchCopy
// performs for signer_account_id inside its CopyFromSlice callback at this
// commit (internal/data/statechanges.go). A non-nil error here aborts the COPY.
func EncodeStateChangeForCopy(sc types.StateChange) error {
	_, err := pgtypeBytesFromNullAddressBytea(sc.SignerAccountID)
	return err
}
