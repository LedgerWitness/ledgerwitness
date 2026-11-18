// Assert the #729 reproduction's before/after behavior. Printing a report
// alone cannot distinguish a successful reproduction from a regression.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const entryUrl = new URL(
  "../../corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/entry.json",
  import.meta.url,
);
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

// TOID of the #729 transaction (d9f78749…) as Horizon testnet reports it in
// the transaction's paging_token: independent of wallet-backend. It exceeds
// Number.MAX_SAFE_INTEGER, so it is compared as a decimal string taken from
// the raw report text; JSON.parse would round it.
export const EXPECTED_SIGNER_TO_ID = "19058303195680768";

// State changes wallet-backend's indexer produced for ledger 4437357 at both
// the pre-fix and fix commits. This is an observed regression baseline, not
// an independent oracle; pinning it stops both reports from shrinking
// together unnoticed.
export const EXPECTED_STATE_CHANGES = 17;

/** Decimal-string to_id values in a raw report, without numeric parsing. */
export function rawToIds(raw) {
  return [...raw.matchAll(/"to_id"\s*:\s*"?(\d+)"?/g)].map((m) => m[1]);
}

export function checkReports(before, after, entry = readJson(entryUrl), raw = null) {
  if (raw) {
    for (const [phase, text] of [["pre-fix", raw.before], ["post-fix", raw.after]]) {
      const ids = rawToIds(text);
      assert.ok(ids.length > 0, `${phase}: no to_id in report`);
      for (const id of ids) {
        assert.equal(id, EXPECTED_SIGNER_TO_ID, `${phase}: signer row to_id is not the #729 transaction's TOID`);
      }
    }
  }
  const probes = entry.expected_decoded_values.probes;
  const changes = entry.expected_state_changes_or_participants.signer_changes;
  assert.ok(changes.length > 0, "manifest must declare expected signer changes");
  for (const [phase, report] of [["pre-fix", before], ["post-fix", after]]) {
    assert.equal(report.ledger, probes.ledger_sequence, `${phase}: wrong ledger`);
    assert.equal(report.transactions, probes.tx_count, `${phase}: wrong transaction count`);
    assert.ok(Number.isSafeInteger(report.state_changes) && report.state_changes > 0,
      `${phase}: state changes must not be empty`);
    assert.ok(Array.isArray(report.signer_rows), `${phase}: signer_rows must be an array`);
    assert.equal(report.signer_rows.length, changes.length, `${phase}: wrong signer row count`);
    for (const change of changes) {
      assert.equal(change.change, "added", "checker currently supports signer additions only");
      const rows = report.signer_rows.filter((row) => row.signer === change.signer);
      assert.equal(rows.length, 1, `${phase}: missing or duplicate full signer key`);
      assert.equal(rows[0].category, "SIGNER", `${phase}: wrong signer category`);
      assert.equal(rows[0].reason, "ADD", `${phase}: wrong signer reason`);
      assert.equal(rows[0].encoded, phase === "post-fix", `${phase}: unexpected conversion result`);
      if (phase === "pre-fix") {
        // #729: the P… key is rejected because its payload is longer than the
        // 32 bytes the column stores. Any other error is not this bug.
        assert.ok(typeof rows[0].error === "string", "pre-fix: missing conversion error");
        assert.ok(rows[0].error.includes(change.signer),
          "pre-fix: conversion error does not name the signer key");
        assert.match(rows[0].error, /\d+-byte key payload/,
          "pre-fix: conversion error is not the payload-length rejection");
      } else {
        assert.ok(!rows[0].error, "post-fix: signer still has a conversion error");
      }
    }
  }
  assert.equal(before.state_changes, after.state_changes, "fix must preserve state changes");
  assert.equal(after.state_changes, EXPECTED_STATE_CHANGES, "state-change count differs from the recorded baseline");
  assert.equal(before.rows_failing_encode, changes.length, "pre-fix: expected signer conversion failure");
  assert.equal(after.rows_failing_encode, 0, "post-fix: every row must convert");
  assert.ok(typeof before.first_encode_error === "string" && before.first_encode_error.length > 0,
    "pre-fix: missing first conversion error");
  assert.ok(!after.first_encode_error, "post-fix: conversion error remains");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [before, after] = process.argv.slice(2);
  assert.ok(before && after, "usage: node check-results.mjs <pre-fix.json> <post-fix.json>");
  const raw = { before: readFileSync(before, "utf8"), after: readFileSync(after, "utf8") };
  checkReports(JSON.parse(raw.before), JSON.parse(raw.after), readJson(entryUrl), raw);
  console.log("wallet-backend #729: pre-fix failure and post-fix success confirmed");
}
