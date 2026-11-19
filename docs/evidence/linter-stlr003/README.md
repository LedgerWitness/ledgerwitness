# Evidence: linter rule STLR003 on wallet-backend

STLR003 reports a function that builds transaction or operation participants
from the envelope alone, with no ledger-meta input (architecture §13.1). The
envelope records what the submitter claimed; the meta records what
stellar-core changed. This document runs the rule on wallet-backend before
and after the fix for exactly this mistake.

## Summary

| wallet-backend commit | Date | STLR003 findings |
|---|---|---|
| `666a64d4`, parent of the fix | 2026-09-25 | **6** |
| `9538b3ca`, "derive Soroban participants from executed meta, not the auth tree" (wallet-backend#753) | 2026-09-28 | **0** |
| `18db88f4`, `main` on 2026-11-18 | 2026-11-18 | **0** |

The rule flags the pre-fix code and runs clean on the fix and on current
wallet-backend, which is the acceptance check the linter needs.

## The upstream fix

Commit `9538b3ca` explains the defect: "The declared authorization tree is
submitter input. The host never checks entries that no require_auth call
matched, so any address in it … can be planted in a victim's history for a
fraction of a cent." The fix takes Soroban participants from the nonce
ledger entries the host writes after a signature verifies, read through
`GetOperationChanges`.

## The six findings at 666a64d4

All six are in `internal/indexer/processors/contract_operations.go`. None of
these functions, nor any same-package helper they call, reads ledger meta:

| Line | Function | What it read |
|---|---|---|
| 53 | `participantsFromInvocationAndSubInvocations` | the authorization tree's invocations |
| 109 | `participantsForAuthEntries` | `SorobanAuthorizationEntry` credentials |
| 151 | `participantsForSorobanOp` | the operation body and the three processors below |
| 252 | `CreateContractV1OpProcessor.Participants` | the create-contract arguments and auth |
| 296 | `CreateContractV2OpProcessor.Participants` | the same, for the V2 arm |
| 340 | `InvokeContractOpProcessor.Participants` | the contract address and auth entries |

Each is code the fix removed or rewrote. Triage: all six are true positives
of the pattern wallet-backend itself fixed.

## Why the fix and current main are clean

At `9538b3ca` and `18db88f4`, `participantsForSorobanOp` calls `op.Changes()`,
which calls `ingest.LedgerTransaction.GetOperationChanges`. The rule follows
that same-package call and counts it as meta input. The classic-operation
`TransactionOperationWrapper.Participants` reads the envelope and reaches
meta through `getSponsor` → `Changes`, so it is not reported either.

## What this run does not show

- **A known false negative.** The same commit also stopped attributing
  envelope-named accounts on *failed* transactions (in
  `GetOperationParticipants`). That function reads meta on its other path, so
  the rule cannot see the problem: it detects functions with no meta input
  at all, not branches that ignore the meta they read.
- **Only one codebase.** A clean run applies to these commits and this rule
  scope only. It is not evidence that wallet-backend has no other
  participant problems.
- **One rule of three.** STLR001 and STLR002 are designs.

## Reproduce

Needs Go 1.26 or later and a clone of stellar/wallet-backend. Test files are
excluded (`-test=false`).

```sh
git clone https://github.com/stellar/wallet-backend ../wallet-backend
make lint-wallet-backend WALLET_BACKEND=../wallet-backend
# STLR003 findings: 6 before the fix, 0 at the fix, 0 at <HEAD>
```

`wallet-backend.log` is the recorded run against `18db88f4`, made with
Go 1.27.4 and golang.org/x/tools v0.53.0.
