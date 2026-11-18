import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { checkReports, rawToIds, EXPECTED_SIGNER_TO_ID } from "./check-results.mjs";

const read = (name) => JSON.parse(readFileSync(new URL(`results/${name}.json`, import.meta.url), "utf8"));
const reports = () => [read("pre-fix-729"), read("post-fix-729")];
const rawText = (name) => readFileSync(new URL(`results/${name}.json`, import.meta.url), "utf8");

test("recorded reports demonstrate both sides of #729", () => {
  checkReports(...reports());
});

test("post-fix conversion failures fail the reproduction", () => {
  const [before, after] = reports();
  after.rows_failing_encode = 1;
  assert.throws(() => checkReports(before, after), /every row must convert/);
});

test("a missing or truncated signer cannot pass", () => {
  for (const replacement of [[], [{ signer: "PBCK2RYY" }]]) {
    const [before, after] = reports();
    after.signer_rows = replacement;
    assert.throws(() => checkReports(before, after), /signer/);
  }
});

test("an empty or wrong-ledger replay cannot pass", () => {
  for (const [field, value] of [["ledger", 1], ["transactions", 0], ["state_changes", 0]]) {
    const [before, after] = reports();
    after[field] = value;
    assert.throws(() => checkReports(before, after));
  }
});

test("pre-fix must still demonstrate the actual conversion failure", () => {
  const [before, after] = reports();
  before.signer_rows[0].encoded = true;
  before.rows_failing_encode = 0;
  assert.throws(() => checkReports(before, after), /unexpected conversion result/);
});

test("an unrelated pre-fix error is not a reproduction of #729", () => {
  const [before, after] = reports();
  before.signer_rows[0].error = "connection refused";
  assert.throws(() => checkReports(before, after), /payload-length|signer key/);
});

test("both reports shrinking together cannot pass", () => {
  const [before, after] = reports();
  before.state_changes = 1;
  after.state_changes = 1;
  assert.throws(() => checkReports(before, after), /baseline/);
});

test("to_id is read exactly as a decimal string and matches Horizon's TOID", () => {
  const raw = { before: rawText("pre-fix-729"), after: rawText("post-fix-729") };
  assert.deepEqual(rawToIds(raw.before), [EXPECTED_SIGNER_TO_ID]);
  checkReports(...reports(), undefined, raw);
  // One digit off is caught; a JSON number would have hidden it.
  const tampered = { ...raw, after: raw.after.replace(EXPECTED_SIGNER_TO_ID, "19058303195680769") };
  assert.throws(() => checkReports(...reports(), undefined, tampered), /TOID/);
});
