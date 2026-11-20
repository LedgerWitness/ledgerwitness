// Go conformance runner: computes the probes in docs/spec/probes.md for every
// corpus entry using go-stellar-sdk's xdr, network and strkey packages.
//
// Usage: go run . <corpus-entries-dir>
package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime/debug"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode"

	"github.com/stellar/go-stellar-sdk/network"
	"github.com/stellar/go-stellar-sdk/xdr"
)

type entry struct {
	ID                string `json:"id"`
	XdrType           string `json:"xdr_type"`
	XdrPath           string `json:"xdr_path"`
	NetworkPassphrase string `json:"network_passphrase"`
	Focus             struct {
		TxHash string `json:"tx_hash"`
	} `json:"focus"`
	DerivedArtifacts []struct {
		XdrPath     string `json:"xdr_path"`
		DerivedFrom string `json:"derived_from"`
	} `json:"derived_artifacts"`
}

type result struct {
	ID                     string            `json:"id"`
	Decode                 *string           `json:"decode"`
	Error                  *string           `json:"error"`
	RoundtripExact         *bool             `json:"roundtrip_exact"`
	EnvelopeDecode         *string           `json:"envelope_decode"`
	EnvelopeRoundtripExact *bool             `json:"envelope_roundtrip_exact"`
	Probes                 map[string]any    `json:"probes"`
	Inputs                 map[string]string `json:"inputs"`
	ElapsedMs              float64           `json:"elapsed_ms"`
}

func ptr[T any](v T) *T { return &v }

// opTypeName turns "OperationTypeSetOptions" into "SET_OPTIONS".
func opTypeName(t xdr.OperationType) string {
	name := strings.TrimPrefix(t.String(), "OperationType")
	var b strings.Builder
	for i, r := range name {
		if unicode.IsUpper(r) && i > 0 {
			b.WriteByte('_')
		}
		b.WriteRune(unicode.ToUpper(r))
	}
	return b.String()
}

func muxed(m xdr.MuxedAccount) (string, *string, error) {
	addr, err := m.GetAddress()
	if err != nil {
		return "", nil, err
	}
	if m.Type != xdr.CryptoKeyTypeKeyTypeMuxedEd25519 {
		return addr, nil, nil
	}
	id, err := m.GetId()
	if err != nil {
		return "", nil, err
	}
	s := strconv.FormatUint(id, 10)
	return addr, &s, nil
}

func trustlineAsset(a xdr.TrustLineAsset) string {
	switch a.Type {
	case xdr.AssetTypeAssetTypePoolShare:
		return "pool:" + hex.EncodeToString(a.LiquidityPoolId[:])
	case xdr.AssetTypeAssetTypeCreditAlphanum4:
		return strings.TrimRight(string(a.AlphaNum4.AssetCode[:]), "\x00") + ":" + a.AlphaNum4.Issuer.Address()
	case xdr.AssetTypeAssetTypeCreditAlphanum12:
		return strings.TrimRight(string(a.AlphaNum12.AssetCode[:]), "\x00") + ":" + a.AlphaNum12.Issuer.Address()
	default:
		return "native"
	}
}

type opMeta struct {
	changes xdr.LedgerEntryChanges
	events  int
}

func operationMeta(m xdr.TransactionMeta) []opMeta {
	var out []opMeta
	switch m.V {
	case 0:
		for _, o := range *m.Operations {
			out = append(out, opMeta{o.Changes, 0})
		}
	case 1:
		for _, o := range m.V1.Operations {
			out = append(out, opMeta{o.Changes, 0})
		}
	case 2:
		for _, o := range m.V2.Operations {
			out = append(out, opMeta{o.Changes, 0})
		}
	case 3:
		events := 0
		if m.V3.SorobanMeta != nil && len(m.V3.Operations) == 1 {
			events = len(m.V3.SorobanMeta.Events)
		}
		for _, o := range m.V3.Operations {
			out = append(out, opMeta{o.Changes, events})
		}
	case 4:
		for _, o := range m.V4.Operations {
			out = append(out, opMeta{o.Changes, len(o.Events)})
		}
	}
	return out
}

// envelopeProbes computes the probes that depend on the focus envelope alone.
func envelopeProbes(env xdr.TransactionEnvelope, passphrase string) (map[string]any, error) {
	hash, err := network.HashTransactionInEnvelope(env, passphrase)
	if err != nil {
		return nil, err
	}
	txSource, txSourceID, err := muxed(env.SourceAccount())
	if err != nil {
		return nil, err
	}
	muxedIDs := []string{}
	if txSourceID != nil {
		muxedIDs = append(muxedIDs, *txSourceID)
	}
	opTypes := []string{}
	opSources := []any{}
	poolIDs := []string{}
	for _, op := range env.Operations() {
		opTypes = append(opTypes, opTypeName(op.Body.Type))
		if op.SourceAccount == nil {
			opSources = append(opSources, nil)
		} else {
			addr, id, err := muxed(*op.SourceAccount)
			if err != nil {
				return nil, err
			}
			opSources = append(opSources, addr)
			if id != nil {
				muxedIDs = append(muxedIDs, *id)
			}
		}
		var dest *xdr.MuxedAccount
		switch op.Body.Type {
		case xdr.OperationTypePayment:
			dest = &op.Body.PaymentOp.Destination
		case xdr.OperationTypePathPaymentStrictReceive:
			dest = &op.Body.PathPaymentStrictReceiveOp.Destination
		case xdr.OperationTypePathPaymentStrictSend:
			dest = &op.Body.PathPaymentStrictSendOp.Destination
		case xdr.OperationTypeAccountMerge:
			dest = op.Body.Destination
		case xdr.OperationTypeChangeTrust:
			line := op.Body.ChangeTrustOp.Line
			if line.Type == xdr.AssetTypeAssetTypePoolShare {
				cp := line.LiquidityPool.ConstantProduct
				id, err := xdr.NewPoolId(cp.AssetA, cp.AssetB, cp.Fee)
				if err != nil {
					poolIDs = append(poolIDs, "error: "+err.Error())
				} else {
					poolIDs = append(poolIDs, hex.EncodeToString(id[:]))
				}
			}
		}
		if dest != nil {
			_, id, err := muxed(*dest)
			if err != nil {
				return nil, err
			}
			if id != nil {
				muxedIDs = append(muxedIDs, *id)
			}
		}
	}
	return map[string]any{
		"tx_hash":            hex.EncodeToString(hash[:]),
		"tx_source":          txSource,
		"tx_source_muxed_id": txSourceID,
		"op_types":           opTypes,
		"op_sources":         opSources,
		"muxed_ids":          muxedIDs,
		"envelope_pool_ids":  poolIDs,
	}, nil
}

// ledgerProbes computes ledger-level probes and the focus transaction's meta
// probes.
func ledgerProbes(lcm xdr.LedgerCloseMeta, focusHash string) (map[string]any, map[string]any, error) {
	ledger := map[string]any{
		"ledger_sequence":  lcm.LedgerSequence(),
		"protocol_version": lcm.ProtocolVersion(),
		"tx_count":         lcm.CountTransactions(),
	}
	if focusHash == "" {
		return ledger, nil, nil
	}
	idx := -1
	for i := 0; i < lcm.CountTransactions(); i++ {
		h := lcm.TransactionHash(i)
		if hex.EncodeToString(h[:]) == focusHash {
			idx = i
			break
		}
	}
	if idx < 0 {
		return ledger, nil, fmt.Errorf("focus transaction not in tx_processing")
	}

	signerSet := map[string]bool{}
	trustlines := []string{}
	eventCounts := []int{}
	for _, om := range operationMeta(lcm.TxApplyProcessing(idx)) {
		eventCounts = append(eventCounts, om.events)
		for _, c := range om.changes {
			var le *xdr.LedgerEntry
			created := false
			switch c.Type {
			case xdr.LedgerEntryChangeTypeLedgerEntryCreated:
				le, created = c.Created, true
			case xdr.LedgerEntryChangeTypeLedgerEntryUpdated:
				le = c.Updated
			case xdr.LedgerEntryChangeTypeLedgerEntryRestored:
				le = c.Restored
			default:
				continue
			}
			switch le.Data.Type {
			case xdr.LedgerEntryTypeAccount:
				for _, s := range le.Data.Account.Signers {
					addr, err := s.Key.GetAddress()
					if err != nil {
						addr = "<unencodable: " + err.Error() + ">"
					}
					signerSet[addr] = true
				}
			case xdr.LedgerEntryTypeTrustline:
				if created {
					tl := le.Data.TrustLine
					trustlines = append(trustlines, tl.AccountId.Address()+"|"+trustlineAsset(tl.Asset))
				}
			}
		}
	}
	signers := []string{}
	for s := range signerSet {
		signers = append(signers, s)
	}
	sort.Strings(signers)
	sort.Strings(trustlines)
	focus := map[string]any{
		"successful":               lcm.TransactionResultPair(idx).Result.Successful(),
		"meta_signer_keys":         signers,
		"meta_created_trustlines":  trustlines,
		"op_contract_event_counts": eventCounts,
	}
	return ledger, focus, nil
}

func runEntry(path string) (r result) {
	r.Probes = map[string]any{}
	// Digests of the exact bytes consumed, so ledgerwitness conform can refuse a report
	// produced from another corpus revision (probe spec 0.2.0).
	r.Inputs = map[string]string{}
	var errs []string
	defer func() {
		if p := recover(); p != nil {
			r.Decode = ptr("panic")
			errs = append(errs, fmt.Sprint(p))
		}
		if len(errs) > 0 {
			r.Error = ptr(strings.Join(errs, "; "))
		}
	}()
	raw, err := os.ReadFile(path)
	if err != nil {
		r.ID = filepath.Base(filepath.Dir(path))
		errs = append(errs, err.Error())
		return r
	}
	r.Inputs["manifest_sha256"] = sha256Hex(raw)
	var e entry
	if err := json.Unmarshal(raw, &e); err != nil {
		r.ID = filepath.Base(filepath.Dir(path))
		errs = append(errs, err.Error())
		return r
	}
	r.ID = e.ID
	dir := filepath.Dir(path)
	start := time.Now()
	focus := map[string]any{}

	if e.XdrType == "LedgerCloseMeta" {
		data, err := os.ReadFile(filepath.Join(dir, e.XdrPath))
		if err != nil {
			errs = append(errs, err.Error())
			return r
		}
		r.Inputs["ledger_sha256"] = sha256Hex(data)
		var lcm xdr.LedgerCloseMeta
		if err := xdr.SafeUnmarshal(data, &lcm); err != nil {
			r.Decode = ptr("error")
			errs = append(errs, "ledger: "+err.Error())
		} else {
			r.Decode = ptr("ok")
			re, err := lcm.MarshalBinary()
			r.RoundtripExact = ptr(err == nil && bytes.Equal(re, data))
			ledger, meta, err := ledgerProbes(lcm, e.Focus.TxHash)
			for k, v := range ledger {
				r.Probes[k] = v
			}
			for k, v := range meta {
				focus[k] = v
			}
			if err != nil {
				errs = append(errs, "ledger probes: "+err.Error())
			}
		}
	}

	for _, d := range e.DerivedArtifacts {
		if d.DerivedFrom != "focus_envelope" {
			continue
		}
		data, err := os.ReadFile(filepath.Join(dir, d.XdrPath))
		if err != nil {
			errs = append(errs, err.Error())
			continue
		}
		r.Inputs["envelope_sha256"] = sha256Hex(data)
		var env xdr.TransactionEnvelope
		if err := xdr.SafeUnmarshal(data, &env); err != nil {
			r.EnvelopeDecode = ptr("error")
			errs = append(errs, "envelope: "+err.Error())
			continue
		}
		r.EnvelopeDecode = ptr("ok")
		re, err := env.MarshalBinary()
		r.EnvelopeRoundtripExact = ptr(err == nil && bytes.Equal(re, data))
		p, err := envelopeProbes(env, e.NetworkPassphrase)
		if err != nil {
			errs = append(errs, "envelope probes: "+err.Error())
		}
		for k, v := range p {
			focus[k] = v
		}
	}
	if len(focus) > 0 {
		r.Probes["focus"] = focus
	}
	r.ElapsedMs = float64(time.Since(start).Microseconds()/100) / 10
	return r
}

func sha256Hex(b []byte) string {
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

// sdkLibrary labels the matrix column with the go-stellar-sdk version this
// binary was actually built against, so a dependency bump cannot pass under
// a stale label.
func sdkLibrary() string {
	const module = "github.com/stellar/go-stellar-sdk"
	if info, ok := debug.ReadBuildInfo(); ok {
		for _, dep := range info.Deps {
			if dep.Path == module {
				if dep.Replace != nil {
					dep = dep.Replace
				}
				return module + " " + dep.Version
			}
		}
	}
	return module + " (unknown version)"
}

func main() {
	root := "corpus/entries"
	if len(os.Args) > 1 {
		root = os.Args[1]
	}
	var paths []string
	_ = filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
		if err == nil && !d.IsDir() && d.Name() == "entry.json" {
			paths = append(paths, p)
		}
		return nil
	})
	sort.Strings(paths)
	results := []result{}
	for _, p := range paths {
		results = append(results, runEntry(p))
	}
	doc := map[string]any{
		"runner":     "go",
		"library":    sdkLibrary(),
		"probe_spec": "0.2.0",
		"results":    results,
	}
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	_ = enc.Encode(doc)
}
