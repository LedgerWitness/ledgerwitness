// Strkey decoder check for reproductions/strkey: prints, for every vector,
// whether go-stellar-sdk's strkey.Decode accepts it and whether it
// re-encodes unchanged.
//
// Usage: go run . <vectors.json>
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/stellar/go-stellar-sdk/strkey"
)

var versions = map[byte]strkey.VersionByte{
	'G': strkey.VersionByteAccountID, 'M': strkey.VersionByteMuxedAccount, 'P': strkey.VersionByteSignedPayload,
	'S': strkey.VersionByteSeed, 'T': strkey.VersionByteHashTx, 'X': strkey.VersionByteHashX,
	'C': strkey.VersionByteContract, 'L': strkey.VersionByteLiquidityPool, 'B': strkey.VersionByteClaimableBalance,
}

func main() {
	var vec []struct {
		Strkey string `json:"strkey"`
	}
	b, _ := os.ReadFile(os.Args[1])
	json.Unmarshal(b, &vec)
	type res struct {
		Strkey    string `json:"strkey"`
		Verdict   string `json:"verdict"`
		Roundtrip bool   `json:"roundtrip,omitempty"`
		Error     string `json:"error,omitempty"`
	}
	var out []res
	for _, v := range vec {
		vb, ok := versions[strings.ToUpper(v.Strkey)[0]]
		if !ok {
			out = append(out, res{Strkey: v.Strkey, Verdict: "unsupported"})
			continue
		}
		raw, err := strkey.Decode(vb, v.Strkey)
		if err != nil {
			out = append(out, res{Strkey: v.Strkey, Verdict: "reject", Error: err.Error()})
			continue
		}
		back, _ := strkey.Encode(vb, raw)
		out = append(out, res{Strkey: v.Strkey, Verdict: "accept", Roundtrip: back == v.Strkey})
	}
	j, _ := json.Marshal(map[string]any{"sdk": "go v0.9.0", "results": out})
	fmt.Println(string(j))
}
