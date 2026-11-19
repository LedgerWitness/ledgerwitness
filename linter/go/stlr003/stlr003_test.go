package stlr003_test

import (
	"testing"

	"golang.org/x/tools/go/analysis/analysistest"

	"ledgerwitness/linter/go/stlr003"
)

func TestSeededCases(t *testing.T) {
	analysistest.Run(t, analysistest.TestData(), stlr003.Analyzer, "a")
}
