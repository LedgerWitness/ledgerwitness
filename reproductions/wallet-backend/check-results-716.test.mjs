import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { checkReports } from "./check-results-716.mjs";

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const entry = () =>
  read("../../corpus/entries/core-emitted/core.testnet.cap40.signed-payload-signer.p28/entry.json");
const reports = () => ["pre-fix-716", "guard-716", "fixed-716"].map((n) => read(`results/${n}.json`));
const signerRow = (r) => r.signer_round_trips[0];

test("recorded reports demonstrate the three phases of #716's signer producer", () => {
  checkReports(...reports(), entry());
});

test("a pre-fix report whose signer survives storage cannot pass", () => {
  const [pre, guard, fixed] = reports();
  pre.signer_round_trips = fixed.signer_round_trips;
  assert.throws(() => checkReports(pre, guard, fixed, entry()), /pre-fix/);
});

test("a pre-fix report that refuses the write is the guard, not the truncation", () => {
  const [pre, guard, fixed] = reports();
  pre.signer_round_trips = guard.signer_round_trips;
  assert.throws(() => checkReports(pre, guard, fixed, entry()), /pre-fix/);
});

test("a fixed report that loses or alters the signer cannot pass", () => {
  for (const mutate of [(r) => (r.signer_round_trips = null), (r) => (signerRow(r).exact = false)]) {
    const [pre, guard, fixed] = reports();
    mutate(fixed);
    assert.throws(() => checkReports(pre, guard, fixed, entry()), /fixed/);
  }
});

test("a replay of another ledger cannot pass", () => {
  const [pre, guard, fixed] = reports();
  guard.ledger = 1;
  assert.throws(() => checkReports(pre, guard, fixed, entry()), /ledger/);
});
