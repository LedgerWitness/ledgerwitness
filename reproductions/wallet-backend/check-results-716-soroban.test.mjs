import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { checkReports } from "./check-results-716-soroban.mjs";

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const entry = () =>
  read("../../corpus/entries/core-emitted/core.testnet.cap27.soroban-muxed-extend-ttl.p30/entry.json");
const reports = () =>
  ["pre-fix-716-soroban", "guard-716-soroban", "fixed-716-soroban"].map((n) => read(`results/${n}.json`));
const muxed = () => entry().expected_decoded_values.probes.focus.op_sources[0];

test("recorded reports demonstrate the three phases of #716's Soroban-source producer", () => {
  checkReports(...reports(), entry());
});

test("a pre-fix report whose muxed participant survives storage cannot pass", () => {
  const [pre, guard, fixed] = reports();
  const row = pre.operation_participants.find((r) => r.participant === muxed());
  Object.assign(row, { read_back: muxed(), read_back_decode_error: undefined, exact: true });
  assert.throws(() => checkReports(pre, guard, fixed, entry()), /pre-fix/);
});

test("a guard report that keeps the muxed form, or refuses it, cannot pass", () => {
  for (const change of [{ read_back: muxed() }, { write_error: "refused" }]) {
    const [pre, guard, fixed] = reports();
    Object.assign(guard.operation_participants.find((r) => r.participant === muxed()), change);
    assert.throws(() => checkReports(pre, guard, fixed, entry()), /guard/);
  }
});

test("a fixed report that still produces the muxed participant cannot pass", () => {
  const [pre, guard, fixed] = reports();
  fixed.operation_participants = guard.operation_participants;
  assert.throws(() => checkReports(pre, guard, fixed, entry()), /fixed/);
});

test("a report of another account or ledger cannot pass", () => {
  for (const mutate of [(r) => (r.account = "GXXX"), (r) => (r.ledger = 1)]) {
    const [pre, guard, fixed] = reports();
    mutate(fixed);
    assert.throws(() => checkReports(pre, guard, fixed, entry()));
  }
});
