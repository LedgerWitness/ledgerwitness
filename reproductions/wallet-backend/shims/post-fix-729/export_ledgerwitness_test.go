package data

import "github.com/stellar/wallet-backend/internal/indexer/types"

// EncodeStateChangeForCopy runs stateChangeCopyRow, which StateChangeModel.
// BatchCopy calls for every row before opening the COPY stream at this commit
// (internal/data/statechanges.go).
func EncodeStateChangeForCopy(sc types.StateChange) error {
	_, err := stateChangeCopyRow(sc)
	return err
}
