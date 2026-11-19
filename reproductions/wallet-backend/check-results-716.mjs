// Assert the #716 (signed-payload producer) behaviour at three commits. The
// expected signer is the one the corpus manifest records core storing, not a
// value taken from wallet-backend.
//
// Usage: node check-results-716.mjs <pre-fix.json> <guard.json> <fixed.json> <entry.json>
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

export function expectedSigner(entry) {
  const [added] = entry.expected_state_changes_or_participants.signer_changes.filter((c) => c.change === "added");
  assert.ok(added?.signer?.startsWith("P"), "manifest records no signed-payload signer");
  return added.signer;
}

function rowFor(name, report, entry, signer) {
  assert.equal(report.ledger, entry.source_url_or_ledger.ledger, `${name}: ledger`);
  const rows = (report.signer_round_trips ?? []).filter((r) => r.signer === signer);
  assert.equal(rows.length, 1, `${name}: the indexer must produce the signer row once`);
  return rows[0];
}

export function checkReports(preFix, guard, fixed, entry) {
  const signer = expectedSigner(entry);

  // #716: written without error, truncated to 33 bytes, unreadable afterwards.
  const before = rowFor("pre-fix", preFix, entry, signer);
  assert.equal(before.write_error, undefined, "pre-fix: the write must succeed silently");
  assert.equal(before.stored_bytes, 33, "pre-fix: version byte plus 32 payload bytes");
  assert.ok(before.read_back && before.read_back !== signer, "pre-fix: the value read back must differ");
  assert.match(before.read_back_decode_error ?? "", /invalid (signed )?payload length/, "pre-fix: read-back must not decode");
  assert.equal(before.exact, false);

  // #693: the same row is refused instead of truncated.
  const guarded = rowFor("guard", guard, entry, signer);
  assert.match(guarded.write_error ?? "", /payload/, "guard: the write must be refused");

  // #729: the whole key is stored and read back unchanged.
  const after = rowFor("fixed", fixed, entry, signer);
  assert.equal(after.write_error, undefined, "fixed: the write must succeed");
  assert.equal(after.exact, true, "fixed: the signer must read back unchanged");
  assert.equal(after.read_back_decode_error, undefined, "fixed: the read-back must decode");
  return { signer, truncatedTo: before.read_back };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [preFix, guard, fixed, entry] = process.argv.slice(2).map(readJson);
  const { signer, truncatedTo } = checkReports(preFix, guard, fixed, entry);
  console.log(
    `wallet-backend #716 (signed-payload producer): ${signer} is stored as ${truncatedTo} (undecodable) ` +
      "before #693, refused at #693, and stored exactly from #729",
  );
}
