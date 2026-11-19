// Prints a ledger's hash as recorded in a public stellar-core history
// archive: the LedgerHeaderHistoryEntry of the ledger, read from the
// checkpoint file that contains it. The archive is published by the
// validator operator and is independent of Horizon and of the data lake, so
// it can anchor ledgers that Horizon no longer serves.
//
// Usage: node archive-hash.mjs <ledger> [archive-url]
//   default archive: SDF's mainnet archive core_live_001
import { gunzipSync } from "node:zlib";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(join(dirname(fileURLToPath(import.meta.url)), "..", "..", "runners", "javascript", "package.json"));
const { xdr } = require("sdk-17.4.0");

const ledger = Number(process.argv[2]);
const archive = process.argv[3] ?? "https://history.stellar.org/prd/core-live/core_live_001";
if (!Number.isInteger(ledger) || ledger < 1) {
  console.error("usage: node archive-hash.mjs <ledger> [archive-url]");
  process.exit(2);
}

// Checkpoints close every 64 ledgers; the file is named after the last one.
const checkpoint = Math.floor(ledger / 64) * 64 + 63;
const hex = checkpoint.toString(16).padStart(8, "0");
const url = `${archive}/ledger/${hex.slice(0, 2)}/${hex.slice(2, 4)}/${hex.slice(4, 6)}/ledger-${hex}.xdr.gz`;
const res = await fetch(url);
if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
const buf = gunzipSync(Buffer.from(await res.arrayBuffer()));

// The file is a stream of XDR records, each prefixed by a 4-byte length
// whose top bit marks the last fragment.
for (let off = 0; off < buf.length; ) {
  const len = buf.readUInt32BE(off) & 0x7fffffff;
  const entry = xdr.LedgerHeaderHistoryEntry.fromXDR(buf.subarray(off + 4, off + 4 + len));
  off += 4 + len;
  if (entry.header.ledgerSeq === ledger) {
    const hash = entry.hash;
    console.log(Buffer.from(typeof hash.toXDR === "function" ? hash.toXDR() : hash).toString("hex"));
    process.exit(0);
  }
}
throw new Error(`ledger ${ledger} not in ${url}`);
