package main

import (
	"fmt"

	"github.com/stellar/go-stellar-sdk/txnbuild"
)

func main() {
	account := "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"
	op := &txnbuild.RevokeSponsorship{
		SponsorshipType: txnbuild.RevokeSponsorshipTypeTrustLine,
		TrustLine: &txnbuild.TrustLineID{
			Account: account,
			Asset:   txnbuild.LiquidityPoolShareTrustLineAsset{LiquidityPoolID: txnbuild.LiquidityPoolId{1, 2, 3}},
		},
	}

	fmt.Println("Validate:", op.Validate())

	xdrOp, err := op.BuildXDR()
	fmt.Println("BuildXDR error:", err)
	fmt.Println("encoded asset type:", xdrOp.Body.RevokeSponsorshipOp.LedgerKey.TrustLine.Asset.Type)

	_, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &txnbuild.SimpleAccount{AccountID: account},
		IncrementSequenceNum: true,
		BaseFee:              txnbuild.MinBaseFee,
		Preconditions:        txnbuild.Preconditions{TimeBounds: txnbuild.NewInfiniteTimeout()},
		Operations:           []txnbuild.Operation{op},
	})
	fmt.Println("NewTransaction:", err)
}
