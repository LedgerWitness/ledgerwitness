// Package stlr003 implements rule STLR003 of the indexer linter
// (docs/architecture.md §13): a function that builds transaction or
// operation participants from the envelope alone, with no ledger-meta input.
//
// The envelope records what the submitter claimed; the meta records what
// stellar-core changed. Participants taken only from the envelope can be
// planted by anyone who submits a transaction (wallet-backend#753).
//
// Scope, deliberately narrow:
//   - candidates are functions and methods whose name contains "participant";
//   - envelope input is any selection on the envelope-side XDR types below, or
//     the Envelope field of ingest.LedgerTransaction;
//   - meta input is any value of a meta-side XDR or ingest type, or a meta
//     field or method of ingest.LedgerTransaction;
//   - both are followed through calls to functions and methods of the same
//     package (helper summaries, computed to a fixed point). Calls into other
//     packages are not followed.
//
// A function that reads any meta is not reported, even if a branch of it
// still attributes envelope-only participants. The rule finds functions with
// no meta input at all; it does not prove that the meta it reads is used.
//
// Suppress a finding with a "//lint:ignore STLR003 <reason>" line in the
// function's doc comment.
//
// The rule runs on packages with type errors (RunDespiteErrors), so that one
// unresolvable import, such as a cgo library that cannot be built where the
// linter runs, does not hide the rest of the package. Expressions without
// type information count as neither input: this can miss a finding but cannot
// create one.
package stlr003

import (
	"go/ast"
	"go/types"
	"regexp"
	"strings"

	"golang.org/x/tools/go/analysis"
	"golang.org/x/tools/go/analysis/passes/inspect"
	"golang.org/x/tools/go/types/typeutil"
)

var Analyzer = &analysis.Analyzer{
	Name:     "stlr003",
	Doc:      "report participant-building functions that read the transaction envelope but no ledger meta",
	URL:      "https://github.com/LedgerWitness/ledgerwitness/blob/main/docs/architecture.md",
	Requires: []*analysis.Analyzer{inspect.Analyzer},
	Run:      run,

	RunDespiteErrors: true,
}

var candidateName = regexp.MustCompile(`(?i)participant`)

// XDR types whose fields and methods are submitter input.
var envelopeTypes = map[string]bool{
	"TransactionEnvelope":         true,
	"TransactionV0Envelope":       true,
	"TransactionV1Envelope":       true,
	"FeeBumpTransactionEnvelope":  true,
	"Transaction":                 true,
	"TransactionV0":               true,
	"FeeBumpTransaction":          true,
	"Operation":                   true,
	"OperationBody":               true,
	"InvokeHostFunctionOp":        true,
	"SorobanAuthorizationEntry":   true,
	"SorobanAuthorizedInvocation": true,
	"SorobanCredentials":          true,
}

// XDR and ingest types that carry what core applied.
var metaTypes = map[string]bool{
	"TransactionMeta":    true,
	"TransactionMetaV3":  true,
	"TransactionMetaV4":  true,
	"OperationMeta":      true,
	"OperationMetaV2":    true,
	"LedgerEntryChanges": true,
	"LedgerEntryChange":  true,
	"ContractEvent":      true,
	"Change":             true, // ingest.Change
}

// Members of ingest.LedgerTransaction that read meta.
var ledgerTransactionMeta = map[string]bool{
	"UnsafeMeta":           true,
	"FeeChanges":           true,
	"PostApplyFeeChanges":  true,
	"GetChanges":           true,
	"GetOperationChanges":  true,
	"GetFeeChanges":        true,
	"GetOperationEvents":   true,
	"GetContractEvents":    true,
	"GetTransactionEvents": true,
	"GetDiagnosticEvents":  true,
}

type summary struct {
	decl     *ast.FuncDecl
	envelope bool
	meta     bool
	callees  []*types.Func
}

func run(pass *analysis.Pass) (any, error) {
	funcs := map[*types.Func]*summary{}
	var order []*types.Func
	for _, file := range pass.Files {
		for _, d := range file.Decls {
			fd, ok := d.(*ast.FuncDecl)
			if !ok || fd.Body == nil {
				continue
			}
			fn, ok := pass.TypesInfo.Defs[fd.Name].(*types.Func)
			if !ok {
				continue
			}
			funcs[fn] = summarize(pass, fd)
			order = append(order, fn)
		}
	}

	// Propagate both inputs through same-package calls until nothing changes.
	for changed := true; changed; {
		changed = false
		for _, fn := range order {
			s := funcs[fn]
			for _, c := range s.callees {
				cs, ok := funcs[c.Origin()]
				if !ok {
					continue
				}
				if cs.envelope && !s.envelope {
					s.envelope, changed = true, true
				}
				if cs.meta && !s.meta {
					s.meta, changed = true, true
				}
			}
		}
	}

	for _, fn := range order {
		s := funcs[fn]
		if !candidateName.MatchString(fn.Name()) || !s.envelope || s.meta || suppressed(s.decl) {
			continue
		}
		pass.Reportf(s.decl.Name.Pos(),
			"STLR003: %s derives participants from the transaction envelope with no ledger-meta input; "+
				"take them from the changes core applied", fn.Name())
	}
	return nil, nil
}

func summarize(pass *analysis.Pass, fd *ast.FuncDecl) *summary {
	s := &summary{decl: fd}
	ast.Inspect(fd.Body, func(n ast.Node) bool {
		switch n := n.(type) {
		case *ast.SelectorExpr:
			if sel, ok := pass.TypesInfo.Selections[n]; ok {
				recv := named(sel.Recv())
				switch {
				case isStellarType(recv, "xdr", envelopeTypes):
					s.envelope = true
				case isStellarType(recv, "ingest", map[string]bool{"LedgerTransaction": true}):
					if n.Sel.Name == "Envelope" {
						s.envelope = true
					} else if ledgerTransactionMeta[n.Sel.Name] {
						s.meta = true
					}
				}
			}
		case *ast.CallExpr:
			if fn, ok := typeutil.Callee(pass.TypesInfo, n).(*types.Func); ok && fn.Pkg() == pass.Pkg {
				s.callees = append(s.callees, fn.Origin())
			}
		}
		if e, ok := n.(ast.Expr); ok {
			if t := pass.TypesInfo.TypeOf(e); t != nil && isMetaType(t) {
				s.meta = true
			}
		}
		return true
	})
	return s
}

// named unwraps pointers and returns the named type, or nil.
func named(t types.Type) *types.Named {
	for {
		switch u := types.Unalias(t).(type) {
		case *types.Pointer:
			t = u.Elem()
		case *types.Named:
			return u
		default:
			return nil
		}
	}
}

func isMetaType(t types.Type) bool {
	for {
		switch u := types.Unalias(t).(type) {
		case *types.Pointer:
			t = u.Elem()
		case *types.Slice:
			t = u.Elem()
		case *types.Named:
			return isStellarType(u, "xdr", metaTypes) || isStellarType(u, "ingest", metaTypes)
		default:
			return false
		}
	}
}

// isStellarType reports whether n is one of names in a Stellar Go package
// whose import path ends in /<pkg> (go-stellar-sdk, or the older stellar/go).
func isStellarType(n *types.Named, pkg string, names map[string]bool) bool {
	if n == nil || n.Obj().Pkg() == nil || !names[n.Obj().Name()] {
		return false
	}
	path := n.Obj().Pkg().Path()
	return strings.HasSuffix(path, "/"+pkg) && strings.Contains(path, "github.com/stellar/")
}

func suppressed(fd *ast.FuncDecl) bool {
	if fd.Doc == nil {
		return false
	}
	for _, c := range fd.Doc.List {
		if strings.HasPrefix(strings.TrimSpace(strings.TrimPrefix(c.Text, "//")), "lint:ignore STLR003 ") {
			return true
		}
	}
	return false
}
