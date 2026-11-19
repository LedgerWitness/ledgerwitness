// ledgerwitness-lint runs the indexer linter rules (docs/architecture.md §13)
// over Go packages.
//
// Usage: ledgerwitness-lint [-test=false] <packages>
package main

import (
	"golang.org/x/tools/go/analysis/multichecker"

	"ledgerwitness/linter/go/stlr003"
)

func main() {
	multichecker.Main(stlr003.Analyzer)
}
