import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { checkReports } from "./check-results-731.mjs";

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const entry = () =>
  read("../../corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30/entry.json");
const reports = () => [read("results/pre-fix-731.json"), read("results/post-fix-731.json")];

test("recorded reports demonstrate both sides of #731", () => {
  checkReports(...reports(), entry());
});

test("a pre-fix report that already records the trustline cannot pass", () => {
  const [before, after] = reports();
  before.trustline_state_changes = after.trustline_state_changes;
  before.created_without_trustline_add = [];
  assert.throws(() => checkReports(before, after, entry()), /pre-fix/);
});

test("a post-fix report without the TRUSTLINE/ADD cannot pass", () => {
  for (const changes of [null, [{ account: entry().focus.tx_hash, reason: "ADD" }]]) {
    const [before, after] = reports();
    after.trustline_state_changes = changes;
    assert.throws(() => checkReports(before, after, entry()), /post-fix/);
  }
});

test("a replay of another ledger, or without the meta trustline, cannot pass", () => {
  for (const mutate of [(r) => (r.ledger = 1), (r) => (r.trustlines_created_in_meta = null)]) {
    const [before, after] = reports();
    mutate(after);
    assert.throws(() => checkReports(before, after, entry()));
  }
});
