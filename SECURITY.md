# Security policy

## Vulnerabilities in this repository

Report vulnerabilities in the toolkit itself (for example a way to make
`ledgerwitness verify` accept a tampered artifact, or to exhaust memory with a crafted
input) through GitHub's private vulnerability reporting on this repository
(Security tab → "Report a vulnerability"). Do not open a public issue.

Please include the command, the input (or how to construct it) and the
observed and expected behaviour. Inputs that only exceed a documented policy
limit (`crates/ledgerwitness/src/input.rs`) are not vulnerabilities.

## Defects found in Stellar software

The corpus exists to find ledger-reading defects in SDKs and indexers. When it
finds one that could be security-relevant (ingestion halts, crashes on
attacker-submitted transactions, data corruption, unbounded resource use):

1. Do not publish the triggering corpus entry, issue or reproduction.
2. Report it privately to the affected project. Projects under the `stellar`
   GitHub organisation route reports through the SDF bug bounty program
   (see the organisation's security policy:
   <https://github.com/stellar/.github/blob/master/SECURITY.md>).
3. Publish the corpus entry and the finding in `docs/findings/` only after the
   maintainers have released a fix or agreed to disclosure.

Already-fixed, publicly disclosed bugs (for example those reproduced under
`reproductions/`) are regression tests and follow the normal contribution
process.

## Scope of the verifier's guarantees

`ledgerwitness verify` checks integrity and provenance as described in
`docs/architecture.md` (section 7). It does not authenticate ledger meta, and
its validator check is a single validator's signature over the proposed
value, not a quorum proof. Do not treat a passing entry as more than that.
