// Assert the #716 (muxed Soroban operation source) behaviour at three
// commits. The muxed source and its base account come from the corpus
// manifest, not from wallet-backend.
//
// Usage: node check-results-716-soroban.mjs <pre-fix.json> <guard.json> <fixed.json> <entry.json>
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const list = (x) => x ?? []; // Go writes an empty slice as null.

export function expectedSource(entry) {
  const focus = entry.expected_decoded_values.probes.focus;
  const [muxed] = focus.op_sources;
  assert.ok(muxed?.startsWith("M"), "manifest records no muxed operation source");
  return { muxed, base: focus.tx_source };
}

function rows(name, report, entry, account) {
  assert.equal(report.ledger, entry.source_url_or_ledger.ledger, `${name}: ledger`);
  assert.equal(report.account, account, `${name}: account`);
  return list(report.operation_participants);
}

export function checkReports(preFix, guard, fixed, entry) {
  const { muxed, base } = expectedSource(entry);

  // #716: the muxed source is a participant, written without error as 33
  // bytes, and unreadable afterwards.
  const before = rows("pre-fix", preFix, entry, base).filter((r) => r.participant === muxed);
  assert.equal(before.length, 1, "pre-fix: the muxed source must be a participant");
  assert.equal(before[0].write_error, undefined, "pre-fix: the write must succeed silently");
  assert.equal(before[0].stored_bytes, 33, "pre-fix: version byte plus 32 payload bytes");
  assert.match(before[0].read_back_decode_error ?? "", /payload length/, "pre-fix: read-back must not decode");

  // #693: the indexer still produces the muxed participant, but the column
  // now stores a muxed address as its base account (the muxed ID is dropped).
  const guarded = rows("guard", guard, entry, base).filter((r) => r.participant === muxed);
  assert.equal(guarded.length, 1, "guard: the muxed source must still be a participant");
  assert.equal(guarded[0].write_error, undefined, "guard: the write must succeed");
  assert.equal(guarded[0].read_back, base, "guard: the muxed source must be stored as its base account");

  // #753: the indexer no longer produces the muxed participant; the base
  // account is stored and read back.
  const after = rows("fixed", fixed, entry, base);
  assert.ok(!after.some((r) => r.participant === muxed), "fixed: the muxed source must not be a participant");
  const g = after.filter((r) => r.participant === base);
  assert.ok(g.length > 0 && g.every((r) => r.exact), "fixed: the base account must round-trip exactly");
  return { muxed, base, truncatedTo: before[0].read_back };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [preFix, guard, fixed, entry] = process.argv.slice(2).map(readJson);
  const { muxed, base, truncatedTo } = checkReports(preFix, guard, fixed, entry);
  console.log(
    `wallet-backend #716 (muxed Soroban operation source): ${muxed} is stored as ${truncatedTo} (undecodable) ` +
      `before #693, stored as ${base} from #693, and no longer produced after #753`,
  );
}
