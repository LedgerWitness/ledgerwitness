// Package xdr is a minimal stand-in for go-stellar-sdk/xdr: only the names
// the STLR003 test cases use.
package xdr

type MuxedAccount struct{ Ed25519 [32]byte }

func (m MuxedAccount) Address() string { return "" }

type TransactionEnvelope struct{ ops []Operation }

func (e TransactionEnvelope) SourceAccount() MuxedAccount  { return MuxedAccount{} }
func (e TransactionEnvelope) FeeBumpAccount() MuxedAccount { return MuxedAccount{} }
func (e TransactionEnvelope) Operations() []Operation      { return e.ops }

type Operation struct {
	SourceAccount *MuxedAccount
	Body          OperationBody
}

type OperationBody struct {
	payment *PaymentOp
	invoke  *InvokeHostFunctionOp
}

func (b OperationBody) MustPaymentOp() PaymentOp                       { return *b.payment }
func (b OperationBody) MustInvokeHostFunctionOp() InvokeHostFunctionOp { return *b.invoke }

type PaymentOp struct{ Destination MuxedAccount }

type InvokeHostFunctionOp struct{ Auth []SorobanAuthorizationEntry }

type SorobanAuthorizationEntry struct{ Credentials SorobanCredentials }

type SorobanCredentials struct{ address string }

func (c SorobanCredentials) Address() string { return c.address }

type LedgerEntryChange struct{ account string }

func (c LedgerEntryChange) Account() string { return c.account }

type LedgerEntryChanges []LedgerEntryChange

type TransactionMeta struct{}
