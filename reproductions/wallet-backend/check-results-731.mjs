// Assert the #731 reproduction's before/after behaviour. The expected
// trustline comes from the corpus manifest (the trustline core created, as
// recorded when the entry was admitted), not from wallet-backend.
//
// Usage: node check-results-731.mjs <pre-fix.json> <post-fix.json> <entry.json>
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

/** The SAC-created trustline the manifest says core recorded, as {account, asset}. */
export function expectedTrustline(entry) {
  const [created] = entry.expected_state_changes_or_participants.trustline_changes.filter(
    (c) => c.change === "created",
  );
  assert.ok(created, "manifest records no created trustline");
  return { account: created.account, asset: created.asset };
}

const same = (t) => (c) => c.account === t.account && c.asset === t.asset;
// Go writes an empty slice as null.
const list = (x) => x ?? [];

export function checkReports(before, after, entry) {
  const want = expectedTrustline(entry);
  const txHash = entry.focus.tx_hash;
  for (const [name, r] of [["pre-fix", before], ["post-fix", after]]) {
    assert.equal(r.ledger, entry.source_url_or_ledger.ledger, `${name}: ledger`);
    const inMeta = list(r.trustlines_created_in_meta).filter(same(want));
    assert.equal(inMeta.length, 1, `${name}: the meta must show core creating the trustline once`);
    assert.equal(inMeta[0].tx_hash, txHash, `${name}: created by the focus transaction`);
  }

  // Before the fix: core created the trustline, the indexer recorded nothing for it.
  assert.ok(list(before.created_without_trustline_add).some(same(want)), "pre-fix: the trustline must lack a TRUSTLINE/ADD");
  assert.ok(
    !list(before.trustline_state_changes).some((c) => c.account === want.account),
    "pre-fix: no TRUSTLINE state change at all for the trustor",
  );

  // After the fix: a TRUSTLINE/ADD for the trustor, and nothing left unrecorded.
  assert.ok(
    list(after.trustline_state_changes).some((c) => c.account === want.account && c.reason === "ADD"),
    "post-fix: the trustor must get a TRUSTLINE/ADD",
  );
  assert.equal(list(after.created_without_trustline_add).length, 0, "post-fix: every created trustline is recorded");
  return want;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [before, after, entry] = process.argv.slice(2).map(readJson);
  const want = checkReports(before, after, entry);
  console.log(
    `wallet-backend #731: trustline ${want.account}|${want.asset} created by core; ` +
      "no TRUSTLINE state change before the fix, TRUSTLINE/ADD after it",
  );
}
